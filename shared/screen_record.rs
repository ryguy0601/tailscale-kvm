#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug)]
pub struct CaptureStartInfo {
    pub session_id: String,
    pub file_path: PathBuf,
    pub backend: String,
}

#[derive(Clone, Debug)]
pub struct StitchedOutputs {
    pub mp4_path: PathBuf,
    pub webp_path: PathBuf,
    pub gif_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StitchOrientation {
    Horizontal,
    Vertical,
}

struct ActiveCaptureProcess {
    child: Child,
    output_path: PathBuf,
    backend: String,
    stop_sentinel: Option<PathBuf>,
    script_path: Option<PathBuf>,
    uses_q_stdin: bool,
}

static ACTIVE_CAPTURES: OnceLock<Mutex<HashMap<String, ActiveCaptureProcess>>> = OnceLock::new();

fn active_captures() -> &'static Mutex<HashMap<String, ActiveCaptureProcess>> {
    ACTIVE_CAPTURES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn sanitize_token(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

pub fn sanitize_filename(raw: &str, fallback: &str) -> String {
    let name_only = Path::new(raw)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .next_back()
        .unwrap_or("");
    let cleaned: String = name_only
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .take(96)
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        fallback.to_string()
    } else {
        cleaned
    }
}

pub fn resolve_repo_root() -> Option<PathBuf> {
    let mut seeds = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(p) = exe.parent() {
            seeds.push(p.to_path_buf());
        }
    }

    for seed in seeds {
        for anc in seed.ancestors() {
            if anc.join("host").join("Cargo.toml").is_file()
                && anc.join("receiver").join("Cargo.toml").is_file()
            {
                return Some(anc.to_path_buf());
            }
        }
    }
    None
}

pub fn recordings_base_dir() -> PathBuf {
    if let Some(root) = resolve_repo_root() {
        return root.join("recordings");
    }
    if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let videos = PathBuf::from(&home).join("Videos");
        if videos.is_dir() {
            return videos.join("tailscale-kvm-demos");
        }
        return PathBuf::from(home).join("tailscale-kvm-demos");
    }
    PathBuf::from("recordings")
}

pub fn demo_pics_dir() -> Option<PathBuf> {
    resolve_repo_root().map(|r| r.join("demoPics"))
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    #[cfg(windows)]
    let exe_name = "ffmpeg.exe";
    #[cfg(not(windows))]
    let exe_name = "ffmpeg";

    let mut check_cmd = Command::new(exe_name);
    check_cmd
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    check_cmd.creation_flags(CREATE_NO_WINDOW);
    if check_cmd.status().map(|s| s.success()).unwrap_or(false) {
        return Some(PathBuf::from(exe_name));
    }

    #[cfg(windows)]
    {
        let mut candidates = vec![
            PathBuf::from("C:\\ffmpeg\\bin\\ffmpeg.exe"),
            PathBuf::from("C:\\ProgramData\\chocolatey\\bin\\ffmpeg.exe"),
        ];
        if let Ok(local_app) = env::var("LOCALAPPDATA") {
            let links = PathBuf::from(&local_app).join("Microsoft\\WinGet\\Links\\ffmpeg.exe");
            candidates.push(links);
            let winget_pkgs = PathBuf::from(&local_app).join("Microsoft\\WinGet\\Packages");
            if let Ok(entries) = fs::read_dir(winget_pkgs) {
                for e in entries.flatten() {
                    let fname = e.file_name().to_string_lossy().to_string();
                    if fname.contains("FFmpeg") || fname.contains("ffmpeg") {
                        if let Ok(sub) = fs::read_dir(e.path()) {
                            for s in sub.flatten() {
                                candidates.push(s.path().join("bin\\ffmpeg.exe"));
                            }
                        }
                    }
                }
            }
        }
        if let Ok(profile) = env::var("USERPROFILE") {
            candidates.push(PathBuf::from(profile).join("scoop\\shims\\ffmpeg.exe"));
        }
        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }
    }

    #[cfg(not(windows))]
    {
        for p in ["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/opt/homebrew/bin/ffmpeg"] {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    None
}

pub fn has_ffmpeg() -> bool {
    find_ffmpeg().is_some()
}

fn command_exists(bin: &str) -> bool {
    let mut cmd = Command::new(bin);
    cmd.arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    if cmd.status().is_ok() {
        return true;
    }
    #[cfg(not(windows))]
    {
        Command::new("which")
            .arg(bin)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        false
    }
}

pub fn detect_capture_backend() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if command_exists("gpu-screen-recorder") {
            return "gpu-screen-recorder";
        }
        if command_exists("wf-recorder") {
            return "wf-recorder";
        }
        if has_ffmpeg() && env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false) {
            return "ffmpeg-x11grab";
        }
        return "none";
    }
    #[cfg(windows)]
    {
        if has_ffmpeg() {
            return "ffmpeg-gdigrab";
        }
        return "win32-gdi-mjpeg";
    }
    #[cfg(target_os = "macos")]
    {
        if has_ffmpeg() {
            return "ffmpeg-avfoundation";
        }
        if Path::new("/usr/sbin/screencapture").is_file() {
            return "screencapture";
        }
        return "none";
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        "none"
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_monitor() -> Option<String> {
    if let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).output() {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = val.as_array() {
                if let Some(focused) = arr.iter().find(|m| {
                    m["focused"].as_bool() == Some(true) && m["disabled"].as_bool() != Some(true)
                }) {
                    if let Some(name) = focused["name"].as_str() {
                        if !name.is_empty() && name != "FALLBACK" {
                            return Some(name.to_string());
                        }
                    }
                }
                if let Some(first) = arr.iter().find(|m| m["disabled"].as_bool() != Some(true)) {
                    if let Some(name) = first["name"].as_str() {
                        if !name.is_empty() && name != "FALLBACK" {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Ok(out) = Command::new("gpu-screen-recorder")
        .arg("--list-monitors")
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if let Some((name, _)) = line.split_once('|') {
                let n = name.trim();
                if !n.is_empty() {
                    return Some(n.to_string());
                }
            }
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn start_linux_capture(
    session_dir: &Path,
    role_prefix: &str,
) -> Result<ActiveCaptureProcess, String> {
    let mp4_path = session_dir.join(format!("{}_linux.mp4", role_prefix));
    let _ = fs::remove_file(&mp4_path);
    let monitor = detect_linux_monitor().unwrap_or_else(|| "focused".to_string());

    if command_exists("gpu-screen-recorder") {
        let mut child = Command::new("gpu-screen-recorder")
            .args([
                "-w",
                &monitor,
                "-cursor",
                "yes",
                "-k",
                "auto",
                "-f",
                "30",
                "-fm",
                "cfr",
                "-fallback-cpu-encoding",
                "yes",
                "-o",
            ])
            .arg(&mp4_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn gpu-screen-recorder: {}", e))?;

        std::thread::sleep(Duration::from_millis(350));
        match child.try_wait() {
            Ok(None) => {
                return Ok(ActiveCaptureProcess {
                    child,
                    output_path: mp4_path,
                    backend: "gpu-screen-recorder".to_string(),
                    stop_sentinel: None,
                    script_path: None,
                    uses_q_stdin: false,
                });
            }
            Ok(Some(status)) => {
                let _ = fs::remove_file(&mp4_path);
                eprintln!(
                    "[!] gpu-screen-recorder exited early ({}), trying next backend...",
                    status
                );
            }
            Err(_) => {}
        }
    }

    if command_exists("wf-recorder") {
        let mut cmd = Command::new("wf-recorder");
        if monitor != "focused" {
            cmd.args(["-o", &monitor]);
        }
        cmd.args(["-r", "30", "-f"])
            .arg(&mp4_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn wf-recorder: {}", e))?;

        std::thread::sleep(Duration::from_millis(350));
        if child.try_wait().map(|o| o.is_none()).unwrap_or(false) {
            return Ok(ActiveCaptureProcess {
                child,
                output_path: mp4_path,
                backend: "wf-recorder".to_string(),
                stop_sentinel: None,
                script_path: None,
                uses_q_stdin: false,
            });
        }
        let _ = child.kill();
    }

    if let (Some(ffmpeg), Ok(display)) = (find_ffmpeg(), env::var("DISPLAY")) {
        if !display.is_empty() {
            let mut child = Command::new(ffmpeg)
                .args([
                    "-y",
                    "-f",
                    "x11grab",
                    "-framerate",
                    "24",
                    "-draw_mouse",
                    "1",
                    "-i",
                    &display,
                    "-vf",
                    "scale='min(1600,iw)':-2",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .arg(&mp4_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to spawn ffmpeg x11grab: {}", e))?;

            std::thread::sleep(Duration::from_millis(300));
            if child.try_wait().map(|o| o.is_none()).unwrap_or(false) {
                return Ok(ActiveCaptureProcess {
                    child,
                    output_path: mp4_path,
                    backend: "ffmpeg-x11grab".to_string(),
                    stop_sentinel: None,
                    script_path: None,
                    uses_q_stdin: true,
                });
            }
            let _ = child.kill();
        }
    }

    Err("No working Linux screen recorder found (install gpu-screen-recorder, wf-recorder, or ffmpeg on X11).".to_string())
}

#[cfg(windows)]
fn build_windows_mjpeg_avi_ps1(out_avi: &Path, stop_sentinel: &Path) -> String {
    let out_esc = out_avi.to_string_lossy().replace('\'', "''");
    let stop_esc = stop_sentinel.to_string_lossy().replace('\'', "''");
    format!(
        r#"$ErrorActionPreference = 'Stop'
$OutAvi = '{out_esc}'
$StopFlag = '{stop_esc}'
Add-Type -ReferencedAssemblies @('System.Drawing','System.Windows.Forms') -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

public static class KvmWinScreenRecorder {{
    [StructLayout(LayoutKind.Sequential)]
    struct POINT {{ public int x; public int y; }}

    [StructLayout(LayoutKind.Sequential)]
    struct CURSORINFO {{
        public int cbSize;
        public int flags;
        public IntPtr hCursor;
        public POINT ptScreenPos;
    }}

    [StructLayout(LayoutKind.Sequential)]
    struct ICONINFO {{
        public bool fIcon;
        public int xHotspot;
        public int yHotspot;
        public IntPtr hbmMask;
        public IntPtr hbmColor;
    }}

    [DllImport("user32.dll")]
    static extern bool GetCursorInfo(ref CURSORINFO pci);

    [DllImport("user32.dll")]
    static extern bool GetIconInfo(IntPtr hIcon, out ICONINFO piconinfo);

    [DllImport("user32.dll")]
    static extern bool DrawIconEx(IntPtr hdc, int xLeft, int yTop, IntPtr hIcon, int cxWidth, int cyHeight, uint istepIfAniCur, IntPtr hbrFlickerFreeDraw, uint diFlags);

    [DllImport("gdi32.dll")]
    static extern bool DeleteObject(IntPtr hObject);

    static ImageCodecInfo GetJpegEncoder() {{
        foreach (var c in ImageCodecInfo.GetImageEncoders()) {{
            if (c.MimeType == "image/jpeg") return c;
        }}
        throw new InvalidOperationException("No JPEG encoder");
    }}

    static void WriteFourCC(BinaryWriter bw, string cc) {{
        byte[] b = Encoding.ASCII.GetBytes(cc);
        bw.Write(b, 0, 4);
    }}

    public static void Record(string outPath, string stopFlagPath, int fps) {{
        Rectangle bounds = Screen.PrimaryScreen.Bounds;
        int srcW = Math.Max(2, bounds.Width);
        int srcH = Math.Max(2, bounds.Height);
        int dstW = Math.Min(1280, srcW);
        int dstH = (int)Math.Round((double)srcH * dstW / srcW);
        dstW = Math.Max(2, dstW - (dstW % 2));
        dstH = Math.Max(2, dstH - (dstH % 2));

        ImageCodecInfo jpegCodec = GetJpegEncoder();
        var encParams = new EncoderParameters(1);
        encParams.Param[0] = new EncoderParameter(System.Drawing.Imaging.Encoder.Quality, 85L);

        var idxOffsets = new List<uint>();
        var idxSizes = new List<uint>();

        using (var fs = new FileStream(outPath, FileMode.Create, FileAccess.Write, FileShare.Read))
        using (var bw = new BinaryWriter(fs)) {{
            WriteFourCC(bw, "RIFF");
            long riffSizePos = fs.Position; bw.Write((uint)0);
            WriteFourCC(bw, "AVI ");

            WriteFourCC(bw, "LIST");
            long hdrlSizePos = fs.Position; bw.Write((uint)0);
            long hdrlStart = fs.Position;
            WriteFourCC(bw, "hdrl");

            WriteFourCC(bw, "avih");
            bw.Write((uint)56);
            bw.Write((uint)(1000000 / fps)); // dwMicroSecPerFrame
            bw.Write((uint)(dstW * dstH * 3)); // dwMaxBytesPerSec
            bw.Write((uint)0); // dwPaddingGranularity
            bw.Write((uint)0x10); // AVIF_HASINDEX
            long avihTotalFramesPos = fs.Position; bw.Write((uint)0);
            bw.Write((uint)0); // dwInitialFrames
            bw.Write((uint)1); // dwStreams
            bw.Write((uint)(dstW * dstH)); // dwSuggestedBufferSize
            bw.Write((uint)dstW);
            bw.Write((uint)dstH);
            bw.Write((uint)0); bw.Write((uint)0); bw.Write((uint)0); bw.Write((uint)0);

            WriteFourCC(bw, "LIST");
            long strlSizePos = fs.Position; bw.Write((uint)0);
            long strlStart = fs.Position;
            WriteFourCC(bw, "strl");

            WriteFourCC(bw, "strh");
            bw.Write((uint)56);
            WriteFourCC(bw, "vids");
            WriteFourCC(bw, "MJPG");
            bw.Write((uint)0); // dwFlags
            bw.Write((ushort)0); // wPriority
            bw.Write((ushort)0); // wLanguage
            bw.Write((uint)0); // dwInitialFrames
            bw.Write((uint)1); // dwScale
            bw.Write((uint)fps); // dwRate
            bw.Write((uint)0); // dwStart
            long strhLengthPos = fs.Position; bw.Write((uint)0);
            bw.Write((uint)(dstW * dstH)); // dwSuggestedBufferSize
            bw.Write((uint)0xFFFFFFFF); // dwQuality
            bw.Write((uint)0); // dwSampleSize
            bw.Write((short)0); bw.Write((short)0); bw.Write((short)dstW); bw.Write((short)dstH);

            WriteFourCC(bw, "strf");
            bw.Write((uint)40);
            bw.Write((uint)40); // biSize
            bw.Write((int)dstW);
            bw.Write((int)dstH);
            bw.Write((ushort)1); // biPlanes
            bw.Write((ushort)24); // biBitCount
            WriteFourCC(bw, "MJPG");
            bw.Write((uint)(dstW * dstH * 3));
            bw.Write((int)0); bw.Write((int)0); bw.Write((uint)0); bw.Write((uint)0);

            long endHdr = fs.Position;
            fs.Position = strlSizePos; bw.Write((uint)(endHdr - strlStart));
            fs.Position = hdrlSizePos; bw.Write((uint)(endHdr - hdrlStart));
            fs.Position = endHdr;

            WriteFourCC(bw, "LIST");
            long moviSizePos = fs.Position; bw.Write((uint)0);
            long moviStart = fs.Position;
            WriteFourCC(bw, "movi");

            using (var srcBmp = new Bitmap(srcW, srcH, PixelFormat.Format32bppRgb))
            using (var dstBmp = new Bitmap(dstW, dstH, PixelFormat.Format24bppRgb))
            using (var gSrc = Graphics.FromImage(srcBmp))
            using (var gDst = Graphics.FromImage(dstBmp)) {{
                gDst.InterpolationMode = InterpolationMode.Bilinear;
                byte[] lastJpeg = null;
                uint frameCount = 0;
                double frameMs = 1000.0 / fps;
                var sw = Stopwatch.StartNew();

                while (!File.Exists(stopFlagPath) && sw.Elapsed.TotalSeconds < 600.0) {{
                    gSrc.CopyFromScreen(bounds.X, bounds.Y, 0, 0, new Size(srcW, srcH), CopyPixelOperation.SourceCopy);

                    CURSORINFO pci = new CURSORINFO();
                    pci.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
                    if (GetCursorInfo(ref pci) && (pci.flags & 1) != 0 && pci.hCursor != IntPtr.Zero) {{
                        ICONINFO ii;
                        int cx = pci.ptScreenPos.x - bounds.X;
                        int cy = pci.ptScreenPos.y - bounds.Y;
                        if (GetIconInfo(pci.hCursor, out ii)) {{
                            cx -= ii.xHotspot;
                            cy -= ii.yHotspot;
                            if (ii.hbmMask != IntPtr.Zero) DeleteObject(ii.hbmMask);
                            if (ii.hbmColor != IntPtr.Zero) DeleteObject(ii.hbmColor);
                        }}
                        IntPtr hdc = gSrc.GetHdc();
                        try {{
                            DrawIconEx(hdc, cx, cy, pci.hCursor, 0, 0, 0, IntPtr.Zero, 3);
                        }} finally {{
                            gSrc.ReleaseHdc(hdc);
                        }}
                    }}

                    if (dstW == srcW && dstH == srcH) {{
                        gDst.DrawImageUnscaled(srcBmp, 0, 0);
                    }} else {{
                        gDst.DrawImage(srcBmp, 0, 0, dstW, dstH);
                    }}

                    using (var ms = new MemoryStream()) {{
                        dstBmp.Save(ms, jpegCodec, encParams);
                        lastJpeg = ms.ToArray();
                    }}

                    long targetFrames = Math.Max(1L, (long)Math.Floor(sw.Elapsed.TotalMilliseconds / frameMs) + 1);
                    long repeats = Math.Min(4L, Math.Max(1L, targetFrames - frameCount));
                    for (long r = 0; r < repeats; r++) {{
                        uint relOff = (uint)(fs.Position - moviStart);
                        WriteFourCC(bw, "00dc");
                        bw.Write((uint)lastJpeg.Length);
                        bw.Write(lastJpeg, 0, lastJpeg.Length);
                        if ((lastJpeg.Length & 1) == 1) bw.Write((byte)0);
                        idxOffsets.Add(relOff);
                        idxSizes.Add((uint)lastJpeg.Length);
                        frameCount++;
                    }}

                    double nextAt = frameCount * frameMs;
                    int waitMs = (int)(nextAt - sw.Elapsed.TotalMilliseconds);
                    if (waitMs > 1) Thread.Sleep(Math.Min(waitMs, 60));
                }}

                long moviEnd = fs.Position;
                WriteFourCC(bw, "idx1");
                bw.Write((uint)(idxOffsets.Count * 16));
                for (int i = 0; i < idxOffsets.Count; i++) {{
                    WriteFourCC(bw, "00dc");
                    bw.Write((uint)0x10); // AVIIF_KEYFRAME
                    bw.Write(idxOffsets[i]);
                    bw.Write(idxSizes[i]);
                }}

                long endPos = fs.Position;
                fs.Position = moviSizePos; bw.Write((uint)(moviEnd - moviStart));
                fs.Position = avihTotalFramesPos; bw.Write(frameCount);
                fs.Position = strhLengthPos; bw.Write(frameCount);
                fs.Position = riffSizePos; bw.Write((uint)(endPos - 8));
                bw.Flush();
            }}
        }}
    }}
}}
"@
[KvmWinScreenRecorder]::Record($OutAvi, $StopFlag, 15)
"#
    )
}

#[cfg(windows)]
fn start_windows_capture(
    session_dir: &Path,
    role_prefix: &str,
) -> Result<ActiveCaptureProcess, String> {
    if let Some(ffmpeg) = find_ffmpeg() {
        let mp4_path = session_dir.join(format!("{}_windows.mp4", role_prefix));
        let _ = fs::remove_file(&mp4_path);
        let mut cmd = Command::new(ffmpeg);
        cmd.args([
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "24",
            "-draw_mouse",
            "1",
            "-i",
            "desktop",
            "-vf",
            "scale='min(1600,iw)':-2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&mp4_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);

        if let Ok(mut child) = cmd.spawn() {
            std::thread::sleep(Duration::from_millis(350));
            if child.try_wait().map(|o| o.is_none()).unwrap_or(false) {
                return Ok(ActiveCaptureProcess {
                    child,
                    output_path: mp4_path,
                    backend: "ffmpeg-gdigrab".to_string(),
                    stop_sentinel: None,
                    script_path: None,
                    uses_q_stdin: true,
                });
            }
            let _ = child.kill();
        }
    }

    let avi_path = session_dir.join(format!("{}_windows.avi", role_prefix));
    let stop_flag = session_dir.join(format!("{}_stop.flag", role_prefix));
    let ps1_path = session_dir.join(format!("{}_capture.ps1", role_prefix));
    let _ = fs::remove_file(&avi_path);
    let _ = fs::remove_file(&stop_flag);
    let ps1_body = build_windows_mjpeg_avi_ps1(&avi_path, &stop_flag);
    fs::write(&ps1_path, ps1_body)
        .map_err(|e| format!("Failed writing Windows capture script: {}", e))?;

    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(&ps1_path)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed launching PowerShell GDI recorder: {}", e))?;

    std::thread::sleep(Duration::from_millis(450));
    if child.try_wait().map(|o| o.is_none()).unwrap_or(true) {
        Ok(ActiveCaptureProcess {
            child,
            output_path: avi_path,
            backend: "win32-gdi-mjpeg".to_string(),
            stop_sentinel: Some(stop_flag),
            script_path: Some(ps1_path),
            uses_q_stdin: false,
        })
    } else {
        Err("Windows PowerShell GDI recorder exited unexpectedly.".to_string())
    }
}

#[cfg(target_os = "macos")]
fn start_macos_capture(
    session_dir: &Path,
    role_prefix: &str,
) -> Result<ActiveCaptureProcess, String> {
    if let Some(ffmpeg) = find_ffmpeg() {
        let mp4_path = session_dir.join(format!("{}_macos.mp4", role_prefix));
        let _ = fs::remove_file(&mp4_path);
        let mut child = Command::new(ffmpeg)
            .args([
                "-y",
                "-f",
                "avfoundation",
                "-capture_cursor",
                "1",
                "-framerate",
                "24",
                "-i",
                "Capture screen 0:none",
                "-vf",
                "scale='min(1600,iw)':-2",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&mp4_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed spawning ffmpeg avfoundation: {}", e))?;

        std::thread::sleep(Duration::from_millis(350));
        if child.try_wait().map(|o| o.is_none()).unwrap_or(false) {
            return Ok(ActiveCaptureProcess {
                child,
                output_path: mp4_path,
                backend: "ffmpeg-avfoundation".to_string(),
                stop_sentinel: None,
                script_path: None,
                uses_q_stdin: true,
            });
        }
        let _ = child.kill();
    }

    let mov_path = session_dir.join(format!("{}_macos.mov", role_prefix));
    let _ = fs::remove_file(&mov_path);
    let mut child = Command::new("/usr/sbin/screencapture")
        .args(["-x", "-v"])
        .arg(&mov_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed spawning macOS screencapture: {}", e))?;

    std::thread::sleep(Duration::from_millis(350));
    if child.try_wait().map(|o| o.is_none()).unwrap_or(false) {
        Ok(ActiveCaptureProcess {
            child,
            output_path: mov_path,
            backend: "screencapture".to_string(),
            stop_sentinel: None,
            script_path: None,
            uses_q_stdin: false,
        })
    } else {
        Err("macOS screencapture exited early (check Screen Recording permission).".to_string())
    }
}

pub fn start_capture(session_id: &str, role_prefix: &str) -> Result<CaptureStartInfo, String> {
    let clean_session = sanitize_token(session_id, "demo_session");
    let clean_role = sanitize_token(role_prefix, "node");
    let session_dir = recordings_base_dir().join(&clean_session);
    fs::create_dir_all(&session_dir)
        .map_err(|e| format!("Failed creating recording dir {:?}: {}", session_dir, e))?;

    // Stop any previous dangling capture under the same key
    if let Ok(mut map) = active_captures().lock() {
        if let Some(mut prev) = map.remove(&clean_session) {
            let _ = prev.child.kill();
            let _ = prev.child.wait();
        }
    }

    #[cfg(target_os = "linux")]
    let proc_info = start_linux_capture(&session_dir, &clean_role)?;
    #[cfg(windows)]
    let proc_info = start_windows_capture(&session_dir, &clean_role)?;
    #[cfg(target_os = "macos")]
    let proc_info = start_macos_capture(&session_dir, &clean_role)?;
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    let proc_info: ActiveCaptureProcess =
        return Err("Unsupported OS for built-in screen recording.".to_string());

    let info = CaptureStartInfo {
        session_id: clean_session.clone(),
        file_path: proc_info.output_path.clone(),
        backend: proc_info.backend.clone(),
    };

    if let Ok(mut map) = active_captures().lock() {
        map.insert(clean_session, proc_info);
    }

    Ok(info)
}

pub fn stop_capture(session_id: &str) -> Result<PathBuf, String> {
    let clean_session = sanitize_token(session_id, "demo_session");
    let proc_opt = active_captures()
        .lock()
        .map_err(|_| "Active captures lock poisoned".to_string())?
        .remove(&clean_session);

    let Some(mut active) = proc_opt else {
        return Err(format!("No active recording for session '{}'", clean_session));
    };

    if let Some(ref flag) = active.stop_sentinel {
        let _ = fs::write(flag, b"stop");
    }

    if active.uses_q_stdin {
        if let Some(mut stdin) = active.child.stdin.take() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }
    }

    #[cfg(not(windows))]
    if !active.uses_q_stdin && active.stop_sentinel.is_none() {
        let pid = active.child.id();
        let _ = Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
    }

    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        match active.child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    #[cfg(not(windows))]
                    {
                        let _ = Command::new("kill")
                            .args(["-INT", &active.child.id().to_string()])
                            .status();
                    }
                    std::thread::sleep(Duration::from_millis(250));
                    let _ = active.child.kill();
                    let _ = active.child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(80));
            }
            Err(_) => break,
        }
    }

    if let Some(ref flag) = active.stop_sentinel {
        let _ = fs::remove_file(flag);
    }
    if let Some(ref ps1) = active.script_path {
        let _ = fs::remove_file(ps1);
    }

    match fs::metadata(&active.output_path) {
        Ok(meta) if meta.len() > 0 => Ok(active.output_path),
        _ => Err(format!(
            "Recording file missing or empty after stop ({:?}, backend={})",
            active.output_path, active.backend
        )),
    }
}

pub fn slot_stitch_order<'a>(
    slot_name: &str,
    host_path: &'a Path,
    recv_path: &'a Path,
    host_label: &'a str,
    recv_label: &'a str,
) -> (StitchOrientation, &'a Path, &'a Path, &'a str, &'a str) {
    match slot_name.trim().to_ascii_lowercase().as_str() {
        "left" => (
            StitchOrientation::Horizontal,
            recv_path,
            host_path,
            recv_label,
            host_label,
        ),
        "up" | "top" => (
            StitchOrientation::Vertical,
            recv_path,
            host_path,
            recv_label,
            host_label,
        ),
        "down" | "bottom" => (
            StitchOrientation::Vertical,
            host_path,
            recv_path,
            host_label,
            recv_label,
        ),
        _ => (
            StitchOrientation::Horizontal,
            host_path,
            recv_path,
            host_label,
            recv_label,
        ),
    }
}

fn escape_drawtext(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn build_stitch_filtergraph(
    orientation: StitchOrientation,
    first_label: &str,
    second_label: &str,
    fps: u32,
    panel_w: u32,
    panel_h: u32,
    with_labels: bool,
) -> String {
    let l1 = escape_drawtext(first_label);
    let l2 = escape_drawtext(second_label);

    let panel_a = format!(
        "[0:v]setpts=PTS-STARTPTS,fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1",
        fps = fps,
        w = panel_w,
        h = panel_h
    );
    let panel_b = format!(
        "[1:v]setpts=PTS-STARTPTS,fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1",
        fps = fps,
        w = panel_w,
        h = panel_h
    );

    let (va, vb) = if with_labels {
        (
            format!(
                "{},drawtext=text='{}':x=14:y=12:fontsize=18:fontcolor=#f0f6fc:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v0]",
                panel_a, l1
            ),
            format!(
                "{},drawtext=text='{}':x=14:y=12:fontsize=18:fontcolor=#00d4be:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v1]",
                panel_b, l2
            ),
        )
    } else {
        (format!("{}[v0]", panel_a), format!("{}[v1]", panel_b))
    };

    match orientation {
        StitchOrientation::Horizontal => format!(
            "{va};{vb};[v0]pad=iw+6:ih:0:0:color=#00d4be@0.35[v0p];[v0p][v1]hstack=inputs=2:shortest=1[outv]"
        ),
        StitchOrientation::Vertical => format!(
            "{va};{vb};[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];[v0p][v1]vstack=inputs=2:shortest=1[outv]"
        ),
    }
}

fn run_ffmpeg_stitch(
    ffmpeg: &Path,
    first_input: &Path,
    second_input: &Path,
    filter: &str,
    extra_args: &[&str],
    output_path: &Path,
) -> Result<(), String> {
    let _ = fs::remove_file(output_path);
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y")
        .arg("-i")
        .arg(first_input)
        .arg("-i")
        .arg(second_input)
        .args(["-filter_complex", filter, "-map", "[outv]"])
        .args(extra_args)
        .arg(output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd
        .output()
        .map_err(|e| format!("Failed executing ffmpeg: {}", e))?;
    if out.status.success() && fs::metadata(output_path).map(|m| m.len() > 0).unwrap_or(false) {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: String = stderr.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        Err(format!("ffmpeg stitch failed ({}): {}", out.status, tail))
    }
}

pub fn stitch_dual_demo(
    session_dir: &Path,
    host_video: &Path,
    recv_video: &Path,
    slot_name: &str,
    host_label: &str,
    recv_label: &str,
) -> Result<StitchedOutputs, String> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| "ffmpeg not installed on this machine".to_string())?;
    if !host_video.is_file() || !recv_video.is_file() {
        return Err(format!(
            "Missing recording input(s): host={:?}, receiver={:?}",
            host_video, recv_video
        ));
    }

    let (orientation, first_in, second_in, first_lbl, second_lbl) =
        slot_stitch_order(slot_name, host_video, recv_video, host_label, recv_label);

    let mp4_path = session_dir.join("dual_demo.mp4");
    let webp_path = session_dir.join("dual_demo.webp");
    let gif_path = session_dir.join("dual_demo.gif");

    let (pw, ph) = match orientation {
        StitchOrientation::Horizontal => (720u32, 450u32),
        StitchOrientation::Vertical => (960u32, 450u32),
    };

    // 1. High-quality MP4 reference
    let mp4_filter_labeled =
        build_stitch_filtergraph(orientation, first_lbl, second_lbl, 24, pw, ph, true);
    let mp4_args = ["-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-pix_fmt", "yuv420p", "-an"];
    if run_ffmpeg_stitch(&ffmpeg, first_in, second_in, &mp4_filter_labeled, &mp4_args, &mp4_path)
        .is_err()
    {
        let mp4_filter_plain =
            build_stitch_filtergraph(orientation, first_lbl, second_lbl, 24, pw, ph, false);
        run_ffmpeg_stitch(&ffmpeg, first_in, second_in, &mp4_filter_plain, &mp4_args, &mp4_path)?;
    }

    // 2. Crisp 24-bit Animated WebP (ideal for my-portfolio & GitHub)
    let webp_filter_labeled =
        build_stitch_filtergraph(orientation, first_lbl, second_lbl, 15, pw, ph, true);
    let webp_args = [
        "-c:v",
        "libwebp_anim",
        "-lossless",
        "0",
        "-compression_level",
        "4",
        "-q:v",
        "78",
        "-loop",
        "0",
        "-preset",
        "default",
        "-an",
    ];
    if run_ffmpeg_stitch(
        &ffmpeg,
        first_in,
        second_in,
        &webp_filter_labeled,
        &webp_args,
        &webp_path,
    )
    .is_err()
    {
        let webp_filter_plain =
            build_stitch_filtergraph(orientation, first_lbl, second_lbl, 15, pw, ph, false);
        if run_ffmpeg_stitch(&ffmpeg, first_in, second_in, &webp_filter_plain, &webp_args, &webp_path)
            .is_err()
        {
            let webp_compat_args = ["-c:v", "libwebp", "-lossless", "0", "-q:v", "78", "-loop", "0", "-an"];
            run_ffmpeg_stitch(
                &ffmpeg,
                first_in,
                second_in,
                &webp_filter_plain,
                &webp_compat_args,
                &webp_path,
            )?;
        }
    }

    // 3. Universal Palette-Optimized Animated GIF
    let base_gif = build_stitch_filtergraph(orientation, first_lbl, second_lbl, 12, 600, 376, true);
    let gif_filter_labeled = format!(
        "{};[outv]split[s0][s1];[s0]palettegen=max_colors=192:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3[gifv]",
        base_gif
    );
    let run_gif = |flt: &str| -> Result<(), String> {
        let _ = fs::remove_file(&gif_path);
        let mut cmd = Command::new(&ffmpeg);
        cmd.arg("-y")
            .arg("-i")
            .arg(first_in)
            .arg("-i")
            .arg(second_in)
            .args(["-filter_complex", flt, "-map", "[gifv]", "-loop", "0", "-an"])
            .arg(&gif_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let out = cmd
            .output()
            .map_err(|e| format!("Failed executing ffmpeg gif: {}", e))?;
        if out.status.success() && fs::metadata(&gif_path).map(|m| m.len() > 0).unwrap_or(false) {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into_owned())
        }
    };

    if run_gif(&gif_filter_labeled).is_err() {
        let plain = format!(
            "{};[outv]split[s0][s1];[s0]palettegen=max_colors=192:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3[gifv]",
            build_stitch_filtergraph(orientation, first_lbl, second_lbl, 12, 600, 376, false)
        );
        let _ = run_gif(&plain);
    }

    Ok(StitchedOutputs {
        mp4_path,
        webp_path,
        gif_path,
    })
}

pub fn export_to_demo_pics(src_file: &Path, dest_filename: &str) -> Result<PathBuf, String> {
    if !src_file.is_file() {
        return Err(format!("Source artifact missing: {:?}", src_file));
    }
    let dir = demo_pics_dir().ok_or_else(|| "Could not locate repository demoPics/ directory".to_string())?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed creating demoPics directory {:?}: {}", dir, e))?;
    let safe_name = sanitize_filename(dest_filename, "02-dual-screen-demo.webp");
    let dest = dir.join(safe_name);
    fs::copy(src_file, &dest)
        .map_err(|e| format!("Failed copying {:?} to {:?}: {}", src_file, dest, e))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_token_and_filename() {
        assert_eq!(sanitize_token("../bad/id?*", "fallback"), "badid");
        assert_eq!(sanitize_token("", "fallback"), "fallback");
        assert_eq!(
            sanitize_filename("../../evil/demo.webp", "safe.webp"),
            "demo.webp"
        );
        assert_eq!(sanitize_filename("..", "safe.webp"), "safe.webp");
    }

    #[test]
    fn test_slot_stitch_spatial_order() {
        let h = Path::new("host.mp4");
        let r = Path::new("recv.mp4");

        let (ori, a, b, la, lb) = slot_stitch_order("right", h, r, "HOST", "RECV");
        assert_eq!(ori, StitchOrientation::Horizontal);
        assert_eq!(a, h);
        assert_eq!(b, r);
        assert_eq!((la, lb), ("HOST", "RECV"));

        let (ori, a, b, _, _) = slot_stitch_order("left", h, r, "HOST", "RECV");
        assert_eq!(ori, StitchOrientation::Horizontal);
        assert_eq!(a, r);
        assert_eq!(b, h);

        let (ori, a, b, _, _) = slot_stitch_order("up", h, r, "HOST", "RECV");
        assert_eq!(ori, StitchOrientation::Vertical);
        assert_eq!(a, r);
        assert_eq!(b, h);

        let (ori, a, b, _, _) = slot_stitch_order("down", h, r, "HOST", "RECV");
        assert_eq!(ori, StitchOrientation::Vertical);
        assert_eq!(a, h);
        assert_eq!(b, r);
    }

    #[test]
    fn test_stitch_filtergraph_builds_horizontal_and_vertical() {
        let flt_h = build_stitch_filtergraph(
            StitchOrientation::Horizontal,
            "WINDOWS HOST",
            "LINUX RECEIVER",
            15,
            720,
            450,
            true,
        );
        assert!(flt_h.contains("hstack=inputs=2:shortest=1[outv]"));
        assert!(flt_h.contains("drawtext=text='WINDOWS HOST'"));

        let flt_v = build_stitch_filtergraph(
            StitchOrientation::Vertical,
            "TOP",
            "BOTTOM",
            15,
            960,
            450,
            false,
        );
        assert!(flt_v.contains("vstack=inputs=2:shortest=1[outv]"));
        assert!(!flt_v.contains("drawtext"));
    }

    #[test]
    fn test_ffmpeg_stitch_dual_demo_end_to_end() {
        let Some(ffmpeg) = find_ffmpeg() else {
            return;
        };
        let tmp_dir = std::env::temp_dir().join(format!("tkvm_stitch_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).expect("create temp stitch dir");

        let clip_a = tmp_dir.join("host.mp4");
        let clip_b = tmp_dir.join("recv.mp4");

        let mk = |out_p: &Path, color: &str| {
            let mut cmd = Command::new(&ffmpeg);
            cmd.args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c={}:s=320x180:d=0.4", color),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(out_p)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.status().map(|s| s.success()).unwrap_or(false)
        };

        if mk(&clip_a, "#13233a") && mk(&clip_b, "#00d4be") {
            let stitched = stitch_dual_demo(
                &tmp_dir,
                &clip_a,
                &clip_b,
                "right",
                "HOST (WINDOWS)",
                "RECEIVER (LINUX)",
            )
            .expect("stitch_dual_demo succeeds");
            assert!(stitched.webp_path.is_file());
            assert!(fs::metadata(&stitched.webp_path).unwrap().len() > 100);
            assert!(stitched.mp4_path.is_file());
            assert!(fs::metadata(&stitched.mp4_path).unwrap().len() > 100);
        }

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
