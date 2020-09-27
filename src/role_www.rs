use super::common::*;

use std::path::PathBuf;

fn role_www(c: &Context) -> Result<()> {
    let log = c.log();

    c.ensure_packages(&["nginx", "gtar"])?;

    info!(log, "creating base dehydrated directories");
    c.ensure_dir("/opt/dehydrated", ROOT, ROOT, 0o750)?;
    c.ensure_dir("/opt/dehydrated/bin", ROOT, ROOT, 0o750)?;
    c.ensure_dir("/opt/dehydrated/lib", ROOT, ROOT, 0o750)?;
    c.ensure_dir("/var/opt/dehydrated", ROOT, ROOT, 0o750)?;

    /*
     * Older OmniOS and OpenIndiana shipped oawk as /usr/bin/awk!  Create a
     * workaround directory that the wrapper can prepend to PATH so that we
     * might correct this lapse.  This is no longer needed for systems that have
     * passed bug 12482, which went back in APR 2020.
     */
    info!(log, "setting up dehydrated workaround PATH");
    c.ensure_dir("/opt/dehydrated/workaround", ROOT, ROOT, 0o750)?;
    c.ensure_symlink("/opt/dehydrated/workaround/awk", "/usr/bin/nawk",
        ROOT, ROOT)?;
    c.ensure_symlink("/opt/dehydrated/workaround/grep", "/usr/bin/ggrep",
        ROOT, ROOT)?;

    let ver = "0.6.5";
    let base = "https://github.com/lukas2511/dehydrated/releases/download";
    let tar = format!("dehydrated-{}.tar.gz", ver);
    let url = format!("{}/v{}/{}", base, ver, tar);
    let md5 = "cedf07369517c317c4e1075540b94699";

    let tf = format!("/var/tmp/{}", tar);

    if !c.exists_file("/opt/dehydrated/lib/dehydrated-0.6.5/README.md")? {
        c.ensure_download(&url, &tf, &md5, HashType::MD5)?;

        info!(log, "extracting archive: {}", tf);
        c.run(&["/opt/local/bin/gtar", "-xz",
            "-C", "/opt/dehydrated/lib",
            "-f", &tf])?;
    }

    /*
     * Install a wrapper which will first chdir(2) to the work directory in
     * "/var/opt/dehydrated" before running the real thing.
     */
    c.ensure_file(c.file("scripts/dehydrated.sh")?,
        "/opt/dehydrated/bin/dehydrated", ROOT, ROOT, 0o755,
        Create::Always)?;

    /*
     * Install the reconfiguration hook:
     */
    c.ensure_file(c.file("scripts/hook.sh")?,
        "/var/opt/dehydrated/hook", ROOT, ROOT, 0o755,
        Create::Always)?;

    c.ensure_file(c.file("config")?,
        "/var/opt/dehydrated/config", ROOT, ROOT, 0o600,
        Create::Always)?;
    c.ensure_file(c.file("domains.txt")?,
        "/var/opt/dehydrated/domains.txt", ROOT, ROOT, 0o600,
        Create::Always)?;

    if let Some(f) = c.file_maybe("hookauthconfig")? {
        c.ensure_file(f, "/var/opt/dehydrated/hookauthconfig",
            ROOT, ROOT, 0o600, Create::Always)?;
    } else {
        c.ensure_removed("/var/opt/dehydrated/hookauthconfig")?;
    }

    /*
     * To use the HTTP challenge mode, we must configure and start the web
     * server so that it may serve our challenges.  In order to allow it to
     * start up, we'll need a temporary self-signed certificate...
     */

    c.ensure_dir("/var/www", ROOT, "www", 0o755)?;
    c.ensure_dir("/var/www/users", ROOT, "www", 0o755)?;
    c.ensure_dir("/var/www/htdocs", ROOT, "www", 0o750)?;
    c.ensure_dir("/var/www/challenges", ROOT, "www", 0o750)?;

    let cfgroot = PathBuf::from("/opt/local/etc/nginx");
    let cfgfile = |n: &str| -> PathBuf {
        let mut r = cfgroot.clone();
        r.push(n);
        r
    };

    /*
     * Copy the base configuration files:
     */
    for n in &["nginx.conf", "ssl.conf"] {
        c.ensure_file(c.file(n)?, cfgfile(n),
            ROOT, ROOT, 0o600, Create::Always)?;
    }

    /*
     * Build a list of access or error log target directories to create:
     */
    let mut logdirs: Vec<PathBuf> = Vec::new();

    for dir in &["sites", "includes"] {
        let path = cfgfile(dir);

        c.ensure_dir(&path, ROOT, ROOT, 0o700)?;

        /*
         * Ensure that all of the files we have are installed.
         */
        let names = if let Some(files) = c.files_maybe(dir)? {
            let mut names: Vec<std::ffi::OsString> = Vec::new();
            for f in &files {
                let n = f.file_name().unwrap();
                names.push(n.to_os_string());

                let mut p = path.clone();
                p.push(n);

                c.ensure_file(f, p, ROOT, ROOT, 0o600, Create::Always)?;

                /*
                 * Scan this file for log file paths:
                 */
                debug!(log, "scanning {} for log configuration", f.display());
                let lines = c.read_lines(f)?.expect("should still exist");
                for l in lines {
                    let t: Vec<_> = l.trim().split_whitespace().collect();
                    if t.len() >= 2 && t[0] == "access_log" && t[1] != "off" {
                        /*
                         * The access_log directive accepts a log file path, and
                         * we need to ensure that the containing directory
                         * exists:
                         */
                        let mut dir = PathBuf::from(t[1]);
                        assert!(dir.pop());

                        if !logdirs.contains(&dir) {
                            info!(log, "file {} configures log directory {}",
                                f.display(), dir.display());
                            logdirs.push(dir);
                        }
                    }
                }
            }
            names
        } else {
            Vec::new()
        };

        /*
         * Finally, remove any files that are no longer present:
         */
        let mut rd = std::fs::read_dir(&path)?;
        while let Some(ent) = rd.next().transpose()? {
            let path = ent.path();
            let n = path.file_name().unwrap();

            if names.iter().any(|nn| nn == n) {
                continue;
            }

            info!(log, "removing old site file \"{}\"", path.display());
            c.ensure_removed(&path)?;
        }
    }

    /*
     * Create any log directories we found in the configuration files:
     */
    for ld in logdirs {
        c.ensure_dir(&ld, ROOT, ROOT, 0o750)?;
    }

    /*
     * Determine the primary domain name for this instance.
     */
    let sname = match c.read_lines(c.file("domains.txt")?)? {
        None => bail!("domains.txt missing for instance"),
        Some(l) => {
            if let Some(x) = l.first() {
                x.trim().split_whitespace().next().unwrap().to_string()
            } else {
                bail!("domains.txt was empty");
            }
        }
    };

    /*
     * Determine whether we need to bootstrap or not:
     */
    let fullchain = format!("/var/opt/dehydrated/certs/{}/fullchain.pem",
        sname);
    let privkey = format!("/var/opt/dehydrated/certs/{}/privkey.pem",
        sname);
    let bootstrap = !c.exists_file(&fullchain)?;

    let link_fullchain = cfgfile("fullchain.pem");
    let link_privkey = cfgfile("privkey.pem");

    if bootstrap {
        /*
         * Bootstrap:
         */
        info!(log, "create temporary bootstrap certificates...");

        c.ensure_removed(&link_fullchain)?;
        c.ensure_removed(&link_privkey)?;

        c.run(&["/opt/local/bin/openssl", "req",
            "-x509",
            "-newkey", "rsa:4096",
            "-keyout", link_privkey.to_str().expect("privkey"),
            "-out", link_fullchain.to_str().expect("fullchain"),
            "-days", "2",
            "-nodes",
            "-sha256",
            "-subj", &format!("/CN={}", sname)])?;

        info!(log, "checking nginx configuration...");
        c.run(&["/opt/local/sbin/nginx", "-t"])?;

        info!(log, "enabling nginx...");
        c.ensure_online("pkgsrc/nginx", true)?;

        info!(log, "bootstrap lets encrypt...");
        loop {
            if let Err(e) = c.run(&["/opt/dehydrated/bin/dehydrated",
                "--accept-terms", "--register"])
            {
                warn!(log, "failed to register (retry): {}", e);
                sleep(5);
                continue;
            }

            if let Err(e) = c.run(&["/opt/dehydrated/bin/dehydrated",
                "--cron"])
            {
                warn!(log, "failed to get certificates (retry): {}", e);
                sleep(5);
                continue;
            }

            info!(log, "certificates obtained!");
            break;
        }

        info!(log, "removing bootstrap certificates...");
        c.ensure_removed(&link_fullchain)?;
        c.ensure_removed(&link_privkey)?;
    }

    c.ensure_symlink(&link_fullchain, fullchain, ROOT, ROOT)?;
    c.ensure_symlink(&link_privkey, privkey, ROOT, ROOT)?;

    info!(log, "checking nginx configuration...");
    c.run(&["/opt/local/sbin/nginx", "-t"])?;

    info!(log, "enabling nginx...");
    c.ensure_online("pkgsrc/nginx", true)?;

    info!(log, "configuring TLS renewal cron job...");
    c.ensure_cron(ROOT, "dehydrated",
        "0 0 * * * /opt/dehydrated/bin/dehydrated --cron")?;

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "www",
        func: role_www,
        instance_posture: InstancePosture::Required,
    })
}
