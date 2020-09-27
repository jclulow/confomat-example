use super::common::*;
use serde::Deserialize;
use std::path::PathBuf;

fn role_pkgsrc(c: &Context) -> Result<()> {
    let log = c.log();

    if let Some(fi) = c.check("/opt/local/bin/pkgin")? {
        if fi.is_user_executable() {
            info!(log, "pkgin already available");
            return Ok(());
        }
    }

    #[derive(Debug, Deserialize)]
    struct Config {
        tar: String,
        sha: String,
        baseurl: String,
    }
    let cfg: Config = c.config()?;

    let f = PathBuf::from(format!("/tmp/{}", cfg.tar));
    let url = format!("{}{}", cfg.baseurl, cfg.tar);

    c.ensure_download(&url, &f, &cfg.sha, HashType::SHA1)?;

    info!(log, "extracting bootstrap {}", cfg.tar);
    c.run(&["/usr/sbin/tar", "-zxpf", f.to_str().unwrap(), "-C", "/"])?;

    c.update_packages()?;

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "pkgsrc",
        func: role_pkgsrc,
        instance_posture: InstancePosture::Prohibited,
    })
}
