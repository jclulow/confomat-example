use super::common::*;

fn role_zfs_data(c: &Context) -> Result<()> {
    let ds = c.data_dataset()?;

    c.ensure_dataset(&ds, &["compress=on", "mountpoint=/data"])?;

    Ok(())
}

pub fn register(confomat: &mut Confomat) -> Result<()> {
    confomat.register(&RoleProvider {
        name: "zfs_data",
        func: role_zfs_data,
        instance_posture: InstancePosture::Prohibited,
    })
}
