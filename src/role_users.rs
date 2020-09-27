use super::common::*;

use std::collections::HashMap;

use serde::Deserialize;


#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
enum UserClass {
    Ops,
    General,
    Service,
}

#[derive(Debug, Deserialize)]
struct FileUser {
    class: UserClass,
    gecos: Option<String>,
    uid: i64,
    group: String,
    home: Option<String>,
    home_create: Option<bool>,
    profiles: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct FileGroup {
    gid: i64,
}

#[derive(Debug, Deserialize)]
struct ConfomatToml {
    #[serde(default)]
    users: HashMap<String, FileUser>,
    #[serde(default)]
    groups: HashMap<String, FileGroup>,
}

#[derive(Debug, PartialEq, Clone)]
struct User {
    name: String,
    uid: i64,
    class: UserClass,
    gecos: String,
    group: String,
    home: String,
    home_create: bool,
    profiles: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
struct Group {
    name: String,
    gid: i64,
}

#[derive(Debug, PartialEq, Clone)]
struct UsersConfig {
    users: HashMap<String, User>,
    groups: HashMap<String, Group>,
}

fn load(toml: ConfomatToml) -> Result<UsersConfig> {
    let mut users = HashMap::new();
    let mut groups = HashMap::new();

    for (k, v) in &toml.users {
        users.insert(k.to_string(), User {
            name: k.to_string(),
            uid: v.uid,
            class: v.class.clone(),
            /*
             * If no particular gecos value is specified, just use the
             * username:
             */
            gecos: v.gecos.as_ref()
                .map_or_else(|| k.to_string(), |s| s.to_string()),
            group: v.group.to_string(),
            /*
             * If no home directory is specified, use "/home/$USER":
             */
            home: v.home.as_ref()
                .map_or_else(|| format!("/home/{}", k), |s| s.to_string()),
            /*
             * If "home_create" does not appear in the file, we
             * default to creating the home directory:
             */
            home_create: v.home_create.unwrap_or(true),
            profiles: v.profiles.as_ref()
                .map_or_else(|| vec![], |v| v.clone()),
        });
    }

    for (k, v) in &toml.groups {
        groups.insert(k.to_string(), Group {
            name: k.to_string(),
            gid: v.gid,
        });
    }

    Ok(UsersConfig {
        users,
        groups,
    })
}

fn role_users(c: &Context) -> Result<()> {
    let log = c.log();

    /*
     * Load configuration from the data directory:
     */
    let cfg = load(c.config()?)?;

    /*
     * Check for UNIX groups...
     */
    info!(log, "processing groups...");
    for g in cfg.groups.values() {
        let g: &Group = g;

        info!(log, "group {} gid {}", g.name, g.gid);

        if illumos::get_group_by_name(&g.name)?.is_some() {
            info!(log, "group {} exists!", &g.name);
        } else {
            info!(log, "group {} not found, creating...", &g.name);
            c.run(&["/usr/sbin/groupadd", "-g", &g.gid.to_string(), &g.name])?;
        }
    }

    /*
     * Check for UNIX users...
     */
    info!(log, "processing users...");
    for u in cfg.users.values() {
        let u: &User = u;

        info!(log, "user {} uid {}", &u.name, &u.uid);

        match u.class {
            /*
             * Service and operator accounts are created on every host.
             */
            UserClass::Ops | UserClass::Service => (),
            /*
             * XXX For now, skip general class users altogether.
             */
            UserClass::General => continue,
        }

        if illumos::get_passwd_by_name(&u.name)?.is_some() {
            info!(log, "user {} exists!", &u.name);
        } else {
            info!(log, "user {} not found, creating...", &u.name);
            c.run(&["/usr/sbin/useradd",
                "-u", &u.uid.to_string(),
                "-g", &u.group,
                "-d", &u.home,
                "-s", "/bin/bash",
                "-c", &u.gecos,
                &u.name])?;
        }

        info!(log, "user {}: setting nopassword...", &u.name);
        c.run(&["/bin/passwd", "-N", &u.name])?;

        /*
         * We take extra care to avoid running "usermod -P" here if we do not
         * need to adjust the profile list.  This tool appears to alter the
         * database whether or not it needed to do so, which causes nscd to drop
         * its cache on the floor for two seconds (to debounce file
         * modifications) and holds up each turn of this loop.
         */
        let uap = if let Some(ua) = illumos::get_user_attr_by_name(&u.name)? {
            info!(log, "user attributes: {:#?}", ua);
            ua.profiles()
        } else {
            info!(log, "no user attributes?");
            Vec::new()
        };

        if uap != u.profiles {
            let profiles = u.profiles.iter().fold("".to_string(), |mut l, p| {
                if !l.is_empty() {
                    l.push_str(",");
                }
                l.push_str(p);
                l
            });

            info!(log, "user {}: fixing profiles...", &u.name);
            c.run(&["/usr/sbin/usermod", "-P", &profiles, &u.name])?;
        }

        if !u.home_create {
            /*
             * No home directory creation required for this user.
             */
            continue;
        }

        match c.homedir()? {
            HomeDir::NFS => {
                /*
                 * We assume, for now at least, that no action is required on a
                 * system with automounted home directories.
                 *
                 * In particular, we won't reach into the NFS home directory and
                 * (re)create any profile files or potentially damage the
                 * contents in any way.
                 */
                continue;
            }
            HomeDir::ZFS(dataset) => {
                /*
                 * We currently only support the simplest possible layout; i.e.,
                 * rpool/home/$USER => /home/$USER.  Confirm that the home
                 * directory path is as expected...
                 */
                let expect = format!("/home/{}", &u.name);
                if u.home != expect {
                    bail!("user {} with unsupported ZFS home \
                        directory path: {}", &u.name, &u.home);
                }

                /*
                 * Ensure that the dataset exists:
                 */
                let dsname = format!("{}/{}", dataset, &u.name);
                c.ensure_dataset(&dsname, &[])?;
            }
            HomeDir::Bare => {
                /*
                 * On systems where neither NFS nor ZFS home directory
                 * management is in use, we will create bare directories
                 * directly under /home.
                 */
            }
        };

        let homepath = |n: &str| -> String {
            format!("{}/{}", &u.home, n)
        };

        /*
         * Now, the local home directory exists in one form or another.  Make
         * sure the contents are acceptable.
         */
        c.ensure_dir(&u.home, &u.name, &u.group, 0o700)?;
        c.ensure_dir(&homepath(".ssh"), &u.name, &u.group, 0o700)?;
        c.ensure_dir(&homepath("bin"), &u.name, &u.group, 0o755)?;

        let sshkeys = c.file_maybe(format!("ssh_keys/{}", &u.name))?;
        if let Some(sshkeys) = sshkeys {
            let dst = homepath(".ssh/authorized_keys");
            c.ensure_file(&sshkeys, &dst, &u.name, &u.group, 0o600,
                Create::Always)?;
        }

        let dotfiles = c.files("dotfiles")?;
        for src in &dotfiles {
            let n = src.file_name().unwrap().to_str().expect("dotfile name");
            let dst = format!("{}/.{}", &u.home, n);

            info!(log, "dotfile: {}", dst);
            c.ensure_file(src, dst, &u.name, &u.group, 0o600,
                Create::IfMissing)?;
        }
    }

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "users",
        func: role_users,
        instance_posture: InstancePosture::Prohibited,
    })
}
