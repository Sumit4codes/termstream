use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod win {
    const ES_CONTINUOUS: u32 = 0x80000000;
    const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
    const ES_DISPLAY_REQUIRED: u32 = 0x00000002;

    extern "system" {
        fn SetThreadExecutionState(es_flags: u32) -> u32;
    }

    pub fn inhibit_display() {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
        }
    }

    pub fn uninhibit_display() {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

pub struct DisplayInhibitor {
    #[allow(dead_code)]
    dbus_child: Option<Child>,
    #[allow(dead_code)]
    systemd_child: Option<Child>,
    last_heartbeat: Instant,
    is_inhibited: bool,
    enabled: bool,
}

impl DisplayInhibitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            dbus_child: None,
            systemd_child: None,
            last_heartbeat: Instant::now() - Duration::from_secs(60),
            is_inhibited: false,
            enabled,
        }
    }

    pub fn inhibit(&mut self) {
        if !self.enabled || self.is_inhibited {
            return;
        }

        self.is_inhibited = true;

        #[cfg(windows)]
        {
            win::inhibit_display();
        }

        #[cfg(target_os = "macos")]
        {
            if self.dbus_child.is_none() {
                let child = Command::new("caffeinate")
                    .arg("-d")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok();
                self.dbus_child = child;
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            // 1. Persistent D-Bus session inhibitor for GNOME (Mutter), KDE Plasma, and FreeDesktop
            if self.dbus_child.is_none() {
                let script = r#"
import time
try:
    from gi.repository import Gio, GLib
    conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    # GNOME SessionManager (flag 8 = Inhibit Idle/Dim, flag 4 = Inhibit Suspend -> 12)
    try:
        conn.call_sync(
            "org.gnome.SessionManager",
            "/org/gnome/SessionManager",
            "org.gnome.SessionManager",
            "Inhibit",
            GLib.Variant("(susu)", ("termstream", 0, "Playing video", 12)),
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
    except Exception:
        pass
    # FreeDesktop ScreenSaver
    try:
        conn.call_sync(
            "org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver",
            "Inhibit",
            GLib.Variant("(ss)", ("termstream", "Playing video")),
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
    except Exception:
        pass
    # FreeDesktop PowerManagement (KDE Plasma / XFCE)
    try:
        conn.call_sync(
            "org.freedesktop.PowerManagement.Inhibit",
            "/org/freedesktop/PowerManagement/Inhibit",
            "org.freedesktop.PowerManagement.Inhibit",
            "Inhibit",
            GLib.Variant("(ss)", ("termstream", "Playing video")),
            None,
            Gio.DBusCallFlags.NONE,
            -1,
            None,
        )
    except Exception:
        pass
except Exception:
    pass

while True:
    time.sleep(3600)
"#;
                let child = Command::new("python3")
                    .args(["-c", script])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok();
                self.dbus_child = child;
            }

            // 2. systemd-inhibit for system-level idle/sleep
            if self.systemd_child.is_none() {
                let child = Command::new("systemd-inhibit")
                    .args([
                        "--what=idle:sleep",
                        "--who=termstream",
                        "--why=Playing video",
                        "sleep",
                        "infinity",
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok();
                self.systemd_child = child;
            }

            self.send_screensaver_reset();
        }
    }

    pub fn uninhibit(&mut self) {
        if !self.is_inhibited {
            return;
        }

        self.is_inhibited = false;

        #[cfg(windows)]
        {
            win::uninhibit_display();
        }

        #[cfg(unix)]
        {
            if let Some(mut child) = self.dbus_child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(mut child) = self.systemd_child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn heartbeat(&mut self) {
        if !self.enabled || !self.is_inhibited {
            return;
        }

        if self.last_heartbeat.elapsed() >= Duration::from_secs(30) {
            self.last_heartbeat = Instant::now();

            #[cfg(all(unix, not(target_os = "macos")))]
            {
                self.send_screensaver_reset();
            }

            #[cfg(windows)]
            {
                win::inhibit_display();
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn send_screensaver_reset(&mut self) {
        let _ = Command::new("xdg-screensaver")
            .arg("reset")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        let _ = Command::new("xset")
            .args(["s", "reset"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    pub fn is_inhibited(&self) -> bool {
        self.is_inhibited
    }
}

impl Drop for DisplayInhibitor {
    fn drop(&mut self) {
        self.uninhibit();
    }
}
