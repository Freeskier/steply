use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        return run_clipboard_command("cmd", &["/C", "clip"], text)
            .or_else(|_| copy_via_osc52(text));
    }

    #[cfg(target_os = "macos")]
    {
        return run_clipboard_command("pbcopy", &[], text).or_else(|_| copy_via_osc52(text));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let candidates: [(&str, &[&str]); 3] = [
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ];
        let mut last_error: Option<io::Error> = None;
        for (program, args) in candidates {
            match run_clipboard_command(program, args, text) {
                Ok(()) => return Ok(()),
                Err(err) => last_error = Some(err),
            }
        }
        return copy_via_osc52(text)
            .map_err(|_| last_error.unwrap_or_else(|| io::Error::other("clipboard unavailable")));
    }

    #[allow(unreachable_code)]
    Err(io::Error::other("unsupported platform for clipboard"))
}

pub fn open_external_url(url: &str) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(url)
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn()?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(io::Error::other("unsupported platform for URL opening"))
}

fn run_clipboard_command(program: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{program} exited with status {status}"
        )))
    }
}

fn copy_via_osc52(text: &str) -> io::Result<()> {
    let payload = STANDARD.encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{payload}\x07");
    let mut out = io::stdout();
    out.write_all(sequence.as_bytes())?;
    out.flush()?;
    Ok(())
}
