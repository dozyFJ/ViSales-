package ua.visales.app;

import android.app.AlertDialog;
import android.app.DownloadManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.provider.Settings;
import android.widget.Toast;
import org.json.JSONObject;
import java.io.InputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.ByteArrayOutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Locale;

final class Updater {
    private static final String ROOT = "https://raw.githubusercontent.com/dozyFJ/ViSales-/downloads/";
    private final ScannerActivity activity;
    private boolean busy;
    private boolean resumed;
    private Uri verifiedApk;
    private Update pending;

    private static final class Update {
        final String file;
        final String name;
        final String sha256;
        final long versionCode;

        Update(JSONObject json) throws Exception {
            versionCode = json.getLong("versionCode");
            file = json.getString("file");
            name = json.getString("version");
            sha256 = json.getString("sha256").toLowerCase(Locale.ROOT);
            if (!file.matches("ViSales-[0-9]+\\.[0-9]+\\.[0-9]+-Android-arm64\\.apk")
                    || !name.matches("[0-9]+\\.[0-9]+\\.[0-9]+")
                    || !sha256.matches("[0-9a-f]{64}")) {
                throw new IllegalArgumentException("Invalid update metadata");
            }
        }
    }

    Updater(ScannerActivity activity) { this.activity = activity; }

    void check(boolean interactive) {
        if (busy) return;
        busy = true;
        new Thread(() -> {
            try {
                HttpURLConnection connection = (HttpURLConnection) new URL(ROOT + "android-update.json").openConnection();
                connection.setConnectTimeout(6000);
                connection.setReadTimeout(6000);
                connection.setInstanceFollowRedirects(false);
                connection.setUseCaches(false);
                JSONObject json;
                try (InputStream stream = connection.getInputStream()) {
                    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                    byte[] buffer = new byte[1024];
                    int read;
                    while ((read = stream.read(buffer)) != -1) {
                        if (bytes.size() + read > 8192) throw new IllegalArgumentException("Update metadata too large");
                        bytes.write(buffer, 0, read);
                    }
                    json = new JSONObject(bytes.toString("UTF-8"));
                } finally {
                    connection.disconnect();
                }
                PackageInfo info = activity.getPackageManager().getPackageInfo(activity.getPackageName(), 0);
                long installed = versionCode(info);
                Update update = json.getLong("versionCode") > installed ? new Update(json) : null;
                activity.runOnUiThread(() -> {
                    busy = false;
                    if (!resumed || activity.isFinishing() || activity.isDestroyed()) return;
                    if (update == null) {
                        if (interactive) message("Установлено актуальну версію ViSales");
                    } else {
                        new AlertDialog.Builder(activity).setTitle("ViSales " + update.name)
                            .setMessage("Доступне оновлення. Завантажити APK?")
                            .setPositiveButton("Завантажити", (dialog, which) -> start(update))
                            .setNegativeButton("Пізніше", null).show();
                    }
                });
            } catch (Exception error) {
                activity.runOnUiThread(() -> {
                    busy = false;
                    if (interactive) message("Не вдалося перевірити оновлення");
                });
            }
        }, "visales-update-check").start();
    }

    private void start(Update update) {
        if (busy) return;
        if (!activity.getPackageManager().canRequestPackageInstalls()) {
            Intent settings = new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                Uri.parse("package:" + activity.getPackageName()));
            try {
                pending = update;
                activity.startActivity(settings);
            } catch (ActivityNotFoundException unavailable) {
                pending = null;
                message("Відкрийте дозвіл на встановлення застосунків для ViSales у налаштуваннях");
            }
            return;
        }
        busy = true;
        new Thread(() -> download(update), "visales-update-download").start();
    }

    void pause() { resumed = false; }

    void resume() {
        resumed = true;
        installWhenResumed();
        Update update = pending;
        pending = null;
        if (update != null && activity.getPackageManager().canRequestPackageInstalls()) start(update);
    }

    private void download(Update update) {
        DownloadManager manager = activity.getSystemService(DownloadManager.class);
        long id = -1;
        try {
            DownloadManager.Request request = new DownloadManager.Request(Uri.parse(ROOT + update.file))
                .setTitle("ViSales " + update.name)
                .setMimeType("application/vnd.android.package-archive")
                .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE);
            id = manager.enqueue(request);
            long deadline = android.os.SystemClock.elapsedRealtime() + 900_000;
            while (android.os.SystemClock.elapsedRealtime() < deadline) {
                int status;
                try (Cursor cursor = manager.query(new DownloadManager.Query().setFilterById(id))) {
                    if (cursor == null || !cursor.moveToFirst()) throw new IllegalStateException("Missing download");
                    status = cursor.getInt(cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS));
                }
                if (status == DownloadManager.STATUS_FAILED) throw new IllegalStateException("Download failed");
                if (status == DownloadManager.STATUS_SUCCESSFUL) {
                    verify(manager, id, update);
                    Uri uri = manager.getUriForDownloadedFile(id);
                    if (uri == null) throw new IllegalStateException("APK URI unavailable");
                    activity.runOnUiThread(() -> {
                        verifiedApk = uri;
                        installWhenResumed();
                    });
                    return;
                }
                Thread.sleep(750);
            }
            throw new IllegalStateException("Download timed out");
        } catch (Exception error) {
            if (id != -1) manager.remove(id);
            activity.runOnUiThread(() -> {
                busy = false;
                message("Не вдалося завантажити або перевірити APK");
            });
        }
    }

    private void verify(DownloadManager manager, long id, Update update) throws Exception {
        File archive = File.createTempFile("visales-update-", ".apk", activity.getCacheDir());
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            try (InputStream stream = new ParcelFileDescriptor.AutoCloseInputStream(manager.openDownloadedFile(id));
                 FileOutputStream copy = new FileOutputStream(archive)) {
                byte[] buffer = new byte[65536];
                int read;
                long length = 0;
                while ((read = stream.read(buffer)) != -1) {
                    length += read;
                    if (length > 200_000_000) throw new IllegalArgumentException("APK too large");
                    digest.update(buffer, 0, read);
                    copy.write(buffer, 0, read);
                }
            }
            StringBuilder hash = new StringBuilder(64);
            for (byte value : digest.digest()) hash.append(String.format(Locale.ROOT, "%02x", value & 255));
            if (!update.sha256.equals(hash.toString())) throw new IllegalArgumentException("APK checksum mismatch");

            PackageManager packages = activity.getPackageManager();
            int flags = Build.VERSION.SDK_INT >= 28 ? PackageManager.GET_SIGNING_CERTIFICATES : PackageManager.GET_SIGNATURES;
            PackageInfo candidate = packages.getPackageArchiveInfo(archive.getPath(), flags);
            PackageInfo installed = packages.getPackageInfo(activity.getPackageName(), flags);
            Signature[] installedSigners = signers(installed);
            if (candidate == null || !installed.packageName.equals(candidate.packageName)
                    || versionCode(candidate) != update.versionCode || versionCode(candidate) <= versionCode(installed)
                    || installedSigners == null || installedSigners.length == 0
                    || !Arrays.equals(installedSigners, signers(candidate))) {
                throw new IllegalArgumentException("APK identity mismatch");
            }
        } finally {
            archive.delete();
        }
    }

    private static long versionCode(PackageInfo info) {
        return Build.VERSION.SDK_INT >= 28 ? info.getLongVersionCode() : info.versionCode;
    }

    private static Signature[] signers(PackageInfo info) {
        if (Build.VERSION.SDK_INT < 28) return info.signatures;
        return info.signingInfo == null ? null : info.signingInfo.getApkContentsSigners();
    }

    private void installWhenResumed() {
        if (verifiedApk == null || !resumed || activity.isFinishing() || activity.isDestroyed()) return;
        Uri uri = verifiedApk;
        verifiedApk = null;
        busy = false;
        try {
            activity.startActivity(new Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION));
        } catch (ActivityNotFoundException | SecurityException unavailable) {
            message("Системний інсталятор Android недоступний");
        }
    }

    private void message(String text) {
        if (!activity.isFinishing() && !activity.isDestroyed()) {
            Toast.makeText(activity, text, Toast.LENGTH_LONG).show();
        }
    }
}
