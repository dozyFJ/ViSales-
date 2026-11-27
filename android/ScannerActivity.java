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
import android.os.SystemClock;
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
    private Button scanButton;
    private final Button[] cameraButtons = new Button[4];
    private int selectedCamera;
    private long scanDeadline;
    private static final int SCAN_WINDOW_MS = 3000;
    private static final String READY_HINT = "Наведіть камеру на штрихкод і натисніть «Сканувати»";
    private final Runnable scanTimeout = () -> {
        scanDeadline = 0;
        hint.setText("Штрихкод не знайдено. Спробуйте ще раз.");
        scanButton.setEnabled(true);
    };
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
        scanDeadline = 0;
        scanner = new Dialog(this);
        scanner.requestWindowFeature(android.view.Window.FEATURE_NO_TITLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setBackgroundColor(PANEL);
        hint = new TextView(this);
        hint.setText(READY_HINT);
        hint.setTextSize(16);
        hint.setTextColor(TEXT);
        hint.setGravity(android.view.Gravity.CENTER);
        layout.addView(hint, new LinearLayout.LayoutParams(-1, dp(52)));

        boolean landscape = getResources().getConfiguration().orientation
            == android.content.res.Configuration.ORIENTATION_LANDSCAPE;
        LinearLayout body = new LinearLayout(this);
        body.setOrientation(landscape ? LinearLayout.HORIZONTAL : LinearLayout.VERTICAL);
        layout.addView(body, new LinearLayout.LayoutParams(-1, 0, 1));
        FrameLayout cameraView = new FrameLayout(this);
        body.addView(cameraView, landscape
            ? new LinearLayout.LayoutParams(0, -1, 1)
            : new LinearLayout.LayoutParams(-1, 0, 1));
        SurfaceView preview = new SurfaceView(this);
        cameraView.addView(preview, new FrameLayout.LayoutParams(-1, -1));
        guide = new ScanGuide(this);
        cameraView.addView(guide, new FrameLayout.LayoutParams(-1, -1));

        LinearLayout controls = new LinearLayout(this);
        controls.setOrientation(LinearLayout.VERTICAL);
        body.addView(controls, new LinearLayout.LayoutParams(landscape ? dp(252) : -1, -2));
        int cameraCount = Camera.getNumberOfCameras();
        selectedCamera = 0;
        Camera.CameraInfo info = new Camera.CameraInfo();
        boolean foundRear = false;
        for (int row = 0; row < 2; row++) {
            LinearLayout choices = new LinearLayout(this);
            for (int column = 0; column < 2; column++) {
                final int id = row * 2 + column;
                String label = "Камера " + (id + 1);
                if (id < cameraCount) {
                    Camera.getCameraInfo(id, info);
                    if (info.facing == Camera.CameraInfo.CAMERA_FACING_BACK && !foundRear) {
                        selectedCamera = id;
                        foundRear = true;
                    }
                    label += info.facing == Camera.CameraInfo.CAMERA_FACING_FRONT
                        ? " · передня" : " · задня";
                } else {
                    label = "Недоступна";
                }
                Button choice = scannerButton(label);
                choice.setEnabled(id < cameraCount);
                choice.setOnClickListener(view -> {
                    if (recognized || !preview.getHolder().getSurface().isValid()) return;
                    if (selectedCamera == id && camera != null) return;
                    selectedCamera = id;
                    updateCameraChoices();
                    startPreview(preview.getHolder(), id);
                });
                cameraButtons[id] = choice;
                choices.addView(choice, controlCell());
            }
            controls.addView(choices);
        }
        updateCameraChoices();

        LinearLayout actions = new LinearLayout(this);
        Button cancel = scannerButton("Скасувати");
        cancel.setOnClickListener(view -> scanner.dismiss());
        actions.addView(cancel, controlCell());
        scanButton = scannerButton("Сканувати");
        styleButton(scanButton, true);
        scanButton.setEnabled(false);
        scanButton.setOnClickListener(view -> {
            if (camera == null || recognized || scanDeadline != 0) return;
            scanDeadline = SystemClock.elapsedRealtime() + SCAN_WINDOW_MS;
            hint.setText("Зчитуємо штрихкод…");
            scanButton.setEnabled(false);
            scanButton.postDelayed(scanTimeout, SCAN_WINDOW_MS);
            guide.invalidate();
        });
        actions.addView(scanButton, controlCell());
        controls.addView(actions);
        scanner.setContentView(layout);
        scanner.setOnDismissListener(dialog -> {
            closeCamera();
            scanner = null;
            guide = null;
            hint = null;
            scanButton = null;
            java.util.Arrays.fill(cameraButtons, null);
            scanResult.set(completedResult);
        });
        preview.getHolder().addCallback(new SurfaceHolder.Callback() {
            @Override public void surfaceCreated(SurfaceHolder holder) {
                if (scanner == null) return;
                if (cameraCount == 0) {
                    hint.setText("Android не надав доступних камер");
                } else {
                    startPreview(holder, selectedCamera);
                }
            }
            @Override public void surfaceChanged(SurfaceHolder holder, int format, int w, int h) {}
            @Override public void surfaceDestroyed(SurfaceHolder holder) { closeCamera(); }
        });
        scanner.getWindow().setBackgroundDrawable(new ColorDrawable(PANEL));
        scanner.getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        scanner.show();
        scanner.getWindow().setLayout(-1, -1);
    }

    private Button scannerButton(String label) {
        Button button = new Button(this);
        button.setText(label);
        button.setAllCaps(false);
        button.setTextSize(13);
        button.setPadding(dp(4), 0, dp(4), 0);
        button.setStateListAnimator(null);
        styleButton(button, false);
        return button;
    }

    private void styleButton(Button button, boolean active) {
        button.setTextColor(new android.content.res.ColorStateList(
            new int[][]{new int[]{-android.R.attr.state_enabled}, new int[0]},
            new int[]{0xff9aa5b6, active ? PANEL : TEXT}));
        button.setBackground(new android.graphics.drawable.RippleDrawable(
            android.content.res.ColorStateList.valueOf(0x32ffffff),
            new ColorDrawable(active ? 0xfff2ca68 : 0xff1d222b), null));
    }

    private LinearLayout.LayoutParams controlCell() {
        LinearLayout.LayoutParams cell = new LinearLayout.LayoutParams(0, dp(48), 1);
        cell.setMargins(dp(2), dp(2), dp(2), dp(2));
        return cell;
    }

    private void updateCameraChoices() {
        for (int id = 0; id < cameraButtons.length; id++) {
            boolean selected = cameraButtons[id].isEnabled() && id == selectedCamera;
            cameraButtons[id].setSelected(selected);
            styleButton(cameraButtons[id], selected);
        }
    }

    private void startPreview(SurfaceHolder holder, int id) {
        closeCamera();
        try {
            camera = Camera.open(id);
            Camera.Parameters parameters = camera.getParameters();
            parameters.setPreviewFormat(ImageFormat.NV21);
            java.util.List<String> focusModes = parameters.getSupportedFocusModes();
            if (focusModes != null && focusModes.contains(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE)) {
                parameters.setFocusMode(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE);
            }
            camera.setParameters(parameters);
            Camera.Size frame = camera.getParameters().getPreviewSize();
            Camera.CameraInfo info = new Camera.CameraInfo();
            Camera.getCameraInfo(id, info);
            int rotation = getWindowManager().getDefaultDisplay().getRotation() * 90;
            int orientation = info.facing == Camera.CameraInfo.CAMERA_FACING_FRONT
                ? (360 - (info.orientation + rotation) % 360) % 360
                : (info.orientation - rotation + 360) % 360;
            boolean swapped = orientation % 180 != 0;
            camera.setDisplayOrientation(orientation);
            camera.setPreviewDisplay(holder);
            MultiFormatReader reader = new MultiFormatReader();
            camera.setPreviewCallback(new Camera.PreviewCallback() {
                private long lastFrame;
                @Override public void onPreviewFrame(byte[] data, Camera source) {
                    long now = SystemClock.elapsedRealtime();
                    if (source != camera || data == null || scanner == null || recognized
                        || scanDeadline == 0 || now >= scanDeadline || now - lastFrame < 180) return;
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
                        scanButton.removeCallbacks(scanTimeout);
                        scanDeadline = 0;
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
            hint.setText(READY_HINT);
            scanButton.setEnabled(true);
            guide.invalidate();
        } catch (java.io.IOException | RuntimeException error) {
            closeCamera();
            hint.setText("Не вдалося відкрити камеру. Виберіть іншу або спробуйте ще раз.");
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
            } else if (scanDeadline != 0) {
                float phase = (android.os.SystemClock.uptimeMillis() % 1600) / 1600f;
                float y = bounds.top + (1 - Math.abs(2 * phase - 1)) * bounds.height();
                canvas.drawLine(bounds.left, y, bounds.right, y, paint);
                postInvalidateOnAnimation();
            }
        }
    }

    private void closeCamera() {
        scanDeadline = 0;
        if (scanButton != null) {
            scanButton.removeCallbacks(scanTimeout);
            scanButton.setEnabled(false);
        }
        if (guide != null) guide.invalidate();
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
