use super::*;

#[test]
fn test_get_env_default() {
    assert_eq!(get_env("NON_EXISTENT_KEY_ABC_123", 42), 42);
}

#[test]
fn test_get_env_int() {
    unsafe {
        std::env::set_var("TEST_INT", "100");
    }
    assert_eq!(get_env("TEST_INT", 50), 100);
    unsafe {
        std::env::remove_var("TEST_INT");
    }
}

#[test]
fn test_get_env_bool_true() {
    unsafe {
        std::env::set_var("TEST_BOOL_TRUE_YES", "yes");
    }
    assert_eq!(get_env("TEST_BOOL_TRUE_YES", false), true);

    unsafe {
        std::env::set_var("TEST_BOOL_TRUE_1", "1");
    }
    assert_eq!(get_env("TEST_BOOL_TRUE_1", false), true);

    unsafe {
        std::env::set_var("TEST_BOOL_TRUE_TRUE", "true");
    }
    assert_eq!(get_env("TEST_BOOL_TRUE_TRUE", false), true);

    unsafe {
        std::env::remove_var("TEST_BOOL_TRUE_YES");
        std::env::remove_var("TEST_BOOL_TRUE_1");
        std::env::remove_var("TEST_BOOL_TRUE_TRUE");
    }
}

#[test]
fn test_get_env_bool_false() {
    unsafe {
        std::env::set_var("TEST_BOOL_FALSE_NO", "no");
    }
    assert_eq!(get_env("TEST_BOOL_FALSE_NO", true), false);

    unsafe {
        std::env::set_var("TEST_BOOL_FALSE_0", "0");
    }
    assert_eq!(get_env("TEST_BOOL_FALSE_0", true), false);

    unsafe {
        std::env::set_var("TEST_BOOL_FALSE_FALSE", "false");
    }
    assert_eq!(get_env("TEST_BOOL_FALSE_FALSE", true), false);

    unsafe {
        std::env::remove_var("TEST_BOOL_FALSE_NO");
        std::env::remove_var("TEST_BOOL_FALSE_0");
        std::env::remove_var("TEST_BOOL_FALSE_FALSE");
    }
}

#[test]
fn test_get_env_string() {
    unsafe {
        std::env::set_var("TEST_STRING", "hello");
    }
    assert_eq!(
        get_env("TEST_STRING", "default".to_string()),
        "hello".to_string()
    );
    unsafe {
        std::env::remove_var("TEST_STRING");
    }
}

#[test]
fn test_get_env_invalid_parse() {
    unsafe {
        std::env::set_var("TEST_INVALID_INT", "not_an_int");
    }
    assert_eq!(get_env("TEST_INVALID_INT", 50), 50);
    unsafe {
        std::env::remove_var("TEST_INVALID_INT");
    }
}
