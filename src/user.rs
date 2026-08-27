use users::get_user_by_uid;

pub struct User {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
}

pub fn get_user() -> User {
    User {
        name: get_user_name(),
        uid: get_user_id(),
        gid: get_group_id(),
    }
}

pub fn get_user_id() -> u32 {
    unsafe { libc::geteuid() }
}

pub fn get_group_id() -> u32 {
    unsafe { libc::getegid() }
}

pub fn get_user_name() -> String {
    let user = get_user_by_uid(get_user_id()).unwrap();

    user.name().to_str().unwrap().to_string()
}
