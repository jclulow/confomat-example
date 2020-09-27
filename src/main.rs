mod common;
use common::*;

mod role_users;
mod role_www;
mod role_base;
mod role_pkgsrc;
mod role_zfs_data;
mod role_local_homedir;

fn main() -> Result<()> {
    let mut confomat = start()?;

    role_users::register(&mut confomat)?;
    role_www::register(&mut confomat)?;
    role_base::register(&mut confomat)?;
    role_pkgsrc::register(&mut confomat)?;
    role_zfs_data::register(&mut confomat)?;
    role_local_homedir::register(&mut confomat)?;

    confomat.apply()?;

    Ok(())
}
