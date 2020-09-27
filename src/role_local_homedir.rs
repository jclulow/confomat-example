use super::common::*;

fn role_local_homedir(c: &Context) -> Result<()> {
    let log = c.log();

    if c.os() != &OS::OmniOS && c.os() != &OS::OpenIndiana {
        bail!("only works on OmniOS and OpenIndiana so far");
    }
    if !c.is_gz() {
        bail!("I don't currently know what to do in a zone...");
    }

    info!(log, "ensuring automounter is disabled for /home");
    c.ensure_removed("/etc/auto_home")?;
    c.ensure_file(c.file("auto_master")?,
        "/etc/auto_master", ROOT, BIN, 0o644,
        Create::Always)?;

    c.ensure_online("svc:/system/filesystem/autofs:default", false)?;
    c.run(&["/usr/sbin/automount", "-v"])?;

    /*
     * XXX rmdir("/home") ?
     */

    assert!(c.is_gz());
    c.ensure_dataset("rpool/home", &["mountpoint=/home"])?;

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "local_homedir",
        func: role_local_homedir,
        instance_posture: InstancePosture::Prohibited,
    })
}
