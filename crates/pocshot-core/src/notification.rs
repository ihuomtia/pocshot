use std::process::Command;

pub fn send_notification(title: &str, message: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send").arg(title).arg(message).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            message, title
        );
        let _ = Command::new("osascript").arg("-e").arg(&script).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "[reflection.assembly]::loadwithpartialname('System.Windows.Forms'); $notify = new-object system.windows.forms.notifyicon; $notify.icon = [system.drawing.systemicons]::information; $notify.visible = $true; $notify.showballoontip(1000, '{}', '{}', [system.windows.forms.tooltipicon]::info)",
            title, message
        );
        let _ = Command::new("powershell")
            .arg("-Command")
            .arg(&script)
            .spawn();
    }
}
