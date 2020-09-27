use super::common::*;

use std::path::PathBuf;

fn role_base(c: &Context) -> Result<()> {
    let log = c.log();

    c.ensure_dir("/root", ROOT, ROOT, 0o700)?;

    enum Action { Remove };
    let root_profile = |srcname: &str, action: Action| -> Result<()> {
        let mut dst = PathBuf::from("/root");
        dst.push(&format!(".{}", srcname));

        match action {
            Action::Remove => {
                c.ensure_removed(dst)?;
            }
        };

        Ok(())
    };

    let dotfiles = c.files("root/dotfiles")?;
    for src in &dotfiles {
        let n = src.file_name().unwrap().to_str().expect("dotfile name");
        let dst = format!("/root/.{}", n);

        info!(log, "dotfile: {}", dst);
        c.ensure_file(src, dst, ROOT, ROOT, 0o600, Create::Always)?;
    }

    root_profile("cshrc", Action::Remove)?;
    root_profile("irbrc", Action::Remove)?;
    root_profile("login", Action::Remove)?;
    root_profile("profile", Action::Remove)?;

    if c.os() == &OS::OmniOS || c.os() == &OS::OpenIndiana {
        /*
         * Install a few packages that are included in the SmartOS base for
         * consistency.
         */
        c.update_packages_ips()?;
        c.ensure_packages_ips(&[
            "system/library/c-runtime",
            "system/header",
            "network/netcat",
            "network/rsync",
            "system/monitoring/arcstat",
        ])?;
    }

    /*
     * Install these from pkgsrc:
     */
    c.ensure_packages(&["tmux", "git"])?;

    if (c.os() == &OS::OmniOS || c.os() == &OS::OpenIndiana) && c.is_gz() {
        info!(log, "configuring NTP client");

        let w = c.ensure_file(c.file("ntp.conf")?,
            "/etc/inet/ntp.conf", ROOT, BIN, 0o644, Create::Always)?;

        c.ensure_online("svc:/network/ntp:default", w)?;
    }

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "base",
        func: role_base,
        instance_posture: InstancePosture::Prohibited,
    })
}
