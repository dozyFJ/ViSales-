package ua.visales.app;

import android.Manifest;
import android.app.Dialog;
import android.app.NativeActivity;
import android.content.pm.PackageManager;
import android.hardware.Camera;
import android.graphics.ImageFormat;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.drawable.ColorDrawable;
import android.content.Context;
import android.view.SurfaceHolder;
import android.view.View;
import android.view.SurfaceView;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.view.WindowManager;
import com.google.zxing.BinaryBitmap;
import com.google.zxing.MultiFormatReader;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.NotFoundException;
import com.google.zxing.PlanarYUVLuminanceSource;
import com.google.zxing.common.HybridBinarizer;

@SuppressWarnings("deprecation")
public final class ScannerActivity extends NativeActivity {
    private static final int CAMERA_PERMISSION = 7;
    private final java.util.concurrent.atomic.AtomicReference<String> scanResult = new java.util.concurrent.atomic.AtomicReference<>();
    private Dialog scanner;
    private Camera camera;
    private ScanGuide guide;
    private TextView hint;
    private boolean recognized;
    private static final float GUIDE_WIDTH = 0.82f;
    private static final float GUIDE_HEIGHT = 0.28f;
    private static final int PANEL = 0xff11141a;
    private static final int TEXT = 0xffeff2f8;
    private String completedResult;
    private final Updater updater = new Updater(this);
    private boolean checkedUpdates;

    public void checkViSalesUpdates() {
        runOnUiThread(() -> updater.check(true));
    }

    @Override protected void onResume() {
        super.onResume();
        updater.resume();
        if (!checkedUpdates) {
            checkedUpdates = true;
            updater.check(false);
        }
    }

    public void startBarcodeScan() {
        runOnUiThread(() -> {
            scanResult.set(null);
            if (scanner != null) return;
            if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[]{Manifest.permission.CAMERA}, CAMERA_PERMISSION);
            } else {
                openScanner();
            }
        });
    }

    public String takeBarcodeScanResult() {
        return scanResult.getAndSet(null);
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] grants) {
        super.onRequestPermissionsResult(request, permissions, grants);
        if (request != CAMERA_PERMISSION) return;
        if (grants.length > 0 && grants[0] == PackageManager.PERMISSION_GRANTED) {
            openScanner();
        } else {
            scanResult.set("error:Надайте доступ до камери в налаштуваннях Android");
        }
    }

    private void openScanner() {
        completedResult = "cancel:";
        recognized = false;
        scanner = new Dialog(this);
        scanner.requestWindowFeature(android.view.Window.FEATURE_NO_TITLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setBackgroundColor(PANEL);
        hint = new TextView(this);
        hint.setText("Помістіть штрихкод у рамку");
        hint.setTextSize(16);
        hint.setTextColor(TEXT);
        hint.setGravity(android.view.Gravity.CENTER);
        layout.addView(hint, new LinearLayout.LayoutParams(-1, dp(52)));

        FrameLayout cameraView = new FrameLayout(this);
        layout.addView(cameraView, new LinearLayout.LayoutParams(-1, 0, 1));
        SurfaceView preview = new SurfaceView(this);
        cameraView.addView(preview, new FrameLayout.LayoutParams(-1, -1));
        guide = new ScanGuide(this);
        cameraView.addView(guide, new FrameLayout.LayoutParams(-1, -1));

        Button cancel = new Button(this);
        cancel.setText("Скасувати");
        cancel.setAllCaps(false);
        cancel.setTextColor(TEXT);
        cancel.setBackground(new android.graphics.drawable.RippleDrawable(
            android.content.res.ColorStateList.valueOf(0x32ffffff),
            new ColorDrawable(0xff1d222b), null));
        cancel.setStateListAnimator(null);
        cancel.setOnClickListener(view -> scanner.dismiss());
        layout.addView(cancel, new LinearLayout.LayoutParams(-1, dp(52)));
        scanner.setContentView(layout);
        scanner.setOnDismissListener(dialog -> {
            closeCamera();
            scanner = null;
            guide = null;
            hint = null;
            scanResult.set(completedResult);
        });
        preview.getHolder().addCallback(new SurfaceHolder.Callback() {
            @Override public void surfaceCreated(SurfaceHolder holder) { startPreview(holder); }
            @Override public void surfaceChanged(SurfaceHolder holder, int format, int w, int h) {}
            @Override public void surfaceDestroyed(SurfaceHolder holder) { closeCamera(); }
        });
        scanner.getWindow().setBackgroundDrawable(new ColorDrawable(PANEL));
        scanner.getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        scanner.show();
        scanner.getWindow().setLayout(-1, -1);
    }

    private void startPreview(SurfaceHolder holder) {
        try {
            camera = Camera.open();
            Camera.Parameters parameters = camera.getParameters();
            parameters.setPreviewFormat(ImageFormat.NV21);
            java.util.List<String> focusModes = parameters.getSupportedFocusModes();
            if (focusModes != null && focusModes.contains(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE)) {
                parameters.setFocusMode(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE);
            }
            camera.setParameters(parameters);
            Camera.Size frame = camera.getParameters().getPreviewSize();
            Camera.CameraInfo info = new Camera.CameraInfo();
            Camera.getCameraInfo(0, info);
            int rotation = getWindowManager().getDefaultDisplay().getRotation() * 90;
            int orientation = (info.orientation - rotation + 360) % 360;
            boolean swapped = orientation % 180 != 0;
            camera.setDisplayOrientation(orientation);
            camera.setPreviewDisplay(holder);
            MultiFormatReader reader = new MultiFormatReader();
            camera.setPreviewCallback(new Camera.PreviewCallback() {
                private long lastFrame;
                @Override public void onPreviewFrame(byte[] data, Camera source) {
                    long now = android.os.SystemClock.elapsedRealtime();
                    if (now - lastFrame < 180 || data == null || scanner == null || recognized) return;
                    lastFrame = now;
                    // The Y plane is first in the NV21 preview. Decode the central area.
                    int w = Math.max(1, (int) (frame.width * (swapped ? GUIDE_HEIGHT : GUIDE_WIDTH)));
                    int h = Math.max(1, (int) (frame.height * (swapped ? GUIDE_WIDTH : GUIDE_HEIGHT)));
                    PlanarYUVLuminanceSource luminance = new PlanarYUVLuminanceSource(
                        data, frame.width, frame.height, (frame.width - w) / 2, (frame.height - h) / 2, w, h, false);
                    try {
                        BinaryBitmap bitmap = new BinaryBitmap(new HybridBinarizer(luminance));
                        String barcode;
                        try {
                            barcode = reader.decode(bitmap).getText();
                        } catch (NotFoundException unreadable) {
                            byte[] gray = luminance.getMatrix();
                            int[] rotated = new int[w * h];
                            for (int y = 0; y < h; y++) {
                                for (int x = 0; x < w; x++) {
                                    int intensity = gray[y * w + x] & 255;
                                    rotated[(w - 1 - x) * h + y] = intensity * 0x010101;
                                }
                            }
                            barcode = reader.decode(new BinaryBitmap(new HybridBinarizer(
                                new RGBLuminanceSource(h, w, rotated)))).getText();
                        }
                        recognized = true;
                        completedResult = "code:" + barcode;
                        guide.invalidate();
                        hint.setText("Штрихкод зчитано");
                        Dialog current = scanner;
                        guide.postDelayed(() -> {
                            if (scanner == current) current.dismiss();
                        }, 180);
                    } catch (NotFoundException unreadable) {
                        reader.reset();
                    }
                }
            });
            camera.startPreview();
        } catch (java.io.IOException | RuntimeException error) {
            completedResult = "error:Не вдалося відкрити камеру";
            if (scanner != null) scanner.dismiss();
        }
    }

    private int dp(int size) {
        return Math.round(size * getResources().getDisplayMetrics().density);
    }

    private final class ScanGuide extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF bounds = new RectF();

        ScanGuide(Context context) {
            super(context);
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        }

        @Override protected void onDraw(Canvas canvas) {
            float width = getWidth();
            float height = getHeight();
            bounds.set(width * (1 - GUIDE_WIDTH) / 2, height * (1 - GUIDE_HEIGHT) / 2,
                width * (1 + GUIDE_WIDTH) / 2, height * (1 + GUIDE_HEIGHT) / 2);
            paint.setStyle(Paint.Style.FILL);
            paint.setColor(0x90141117);
            canvas.drawRect(0, 0, width, bounds.top, paint);
            canvas.drawRect(0, bounds.bottom, width, height, paint);
            canvas.drawRect(0, bounds.top, bounds.left, bounds.bottom, paint);
            canvas.drawRect(bounds.right, bounds.top, width, bounds.bottom, paint);
            int accent = recognized ? 0xff8dc771 : 0xfff2ca68;
            paint.setColor(accent);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(dp(2));
            canvas.drawRect(bounds, paint);
            if (recognized) {
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(0x388dc771);
                canvas.drawRect(bounds, paint);
            } else {
                float phase = (android.os.SystemClock.uptimeMillis() % 1600) / 1600f;
                float y = bounds.top + (1 - Math.abs(2 * phase - 1)) * bounds.height();
                canvas.drawLine(bounds.left, y, bounds.right, y, paint);
                postInvalidateOnAnimation();
            }
        }
    }

    private void closeCamera() {
        if (camera == null) return;
        camera.setPreviewCallback(null);
        camera.release();
        camera = null;
    }

    @Override protected void onPause() {
        updater.pause();
        if (scanner != null) scanner.dismiss();
        super.onPause();
    }
}
