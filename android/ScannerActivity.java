package ua.visales.app;

import android.Manifest;
import android.app.Dialog;
import android.app.NativeActivity;
import android.content.pm.PackageManager;
import android.hardware.Camera;
import android.graphics.ImageFormat;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.widget.Button;
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
        scanner = new Dialog(this);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        TextView hint = new TextView(this);
        hint.setText("Наведіть камеру на штрихкод");
        layout.addView(hint);
        SurfaceView preview = new SurfaceView(this);
        layout.addView(preview, new LinearLayout.LayoutParams(-1, 0, 1));
        Button cancel = new Button(this);
        cancel.setText("Скасувати");
        cancel.setOnClickListener(view -> scanner.dismiss());
        layout.addView(cancel);
        scanner.setContentView(layout);
        scanner.setOnDismissListener(dialog -> {
            closeCamera();
            scanner = null;
            scanResult.set(completedResult);
        });
        preview.getHolder().addCallback(new SurfaceHolder.Callback() {
            @Override public void surfaceCreated(SurfaceHolder holder) { startPreview(holder); }
            @Override public void surfaceChanged(SurfaceHolder holder, int format, int w, int h) {}
            @Override public void surfaceDestroyed(SurfaceHolder holder) { closeCamera(); }
        });
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
            camera.setDisplayOrientation((info.orientation - rotation + 360) % 360);
            camera.setPreviewDisplay(holder);
            MultiFormatReader reader = new MultiFormatReader();
            camera.setPreviewCallback(new Camera.PreviewCallback() {
                private long lastFrame;
                @Override public void onPreviewFrame(byte[] data, Camera source) {
                    long now = android.os.SystemClock.elapsedRealtime();
                    if (now - lastFrame < 180 || data == null || scanner == null) return;
                    lastFrame = now;
                    // The Y plane is first in the NV21 preview. Decode the central area.
                    int w = frame.width * 4 / 5;
                    int h = frame.height * 3 / 5;
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
                        completedResult = "code:" + barcode;
                        scanner.dismiss();
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
