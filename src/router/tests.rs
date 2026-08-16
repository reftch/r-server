use super::*;
use crate::core::connection::ConnectionMetadata;
use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use std::str::FromStr;

fn hello_handler<T>(_req: &Request, res: &mut Response<'_, T>) {
    res.body = "Hello, World!".to_string().into();
}

fn param_handler<T>(req: &Request, res: &mut Response<'_, T>) {
    let name = req.param("name").unwrap();
    res.body = format!("Hello, {}!", name).into();
}

#[test]
fn test_add_and_route_basic() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/", hello_handler);
    let mut req = Request {
        method: "GET",
        path: "/",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };

    let handler = router.route(&mut req).expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req, &mut res);
    assert_eq!(res.status, Status::Ok);
    assert_eq!(res.body, "Hello, World!".as_bytes());
}

#[test]
fn test_route_with_params() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/user/:name", param_handler);

    let mut req = Request {
        method: "GET",
        path: "/user/alice",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };

    let handler = router.route(&mut req).expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req, &mut res);
    assert_eq!(res.status, Status::Ok);
    assert_eq!(res.body, "Hello, alice!".as_bytes());
    assert_eq!(req.param("name").unwrap(), "alice");
}

#[test]
fn test_route_with_query_params() {
    let mut router: Router<()> = Router::new();
    let buf = b"GET /api/v1/inc/2?tex=1 HTTP/1.1\r\n\r\n";
    let mut req_from_buf = Request::parse(buf).expect("Should parse");

    router.add_route(Method::GET, "/api/v1/inc/:id", |req, res| {
        let id = req.param("id").unwrap();
        res.body = format!("ID is {}, query tex is {}", id, req.query("tex").unwrap()).into();
    });

    let handler = router
        .route(&mut req_from_buf)
        .expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req_from_buf, &mut res);
    assert_eq!(res.body, "ID is 2, query tex is 1".as_bytes());
}

#[test]
fn test_route_query_params() {
    let mut router: Router<()> = Router::new();
    // Note: Keeping your spelling of 'longtitude' as per the input buffer
    let buf = b"GET /api/v1/temperature?latitude=1.1&longtitude=2.2 HTTP/1.1\r\n\r\n";
    let mut req_from_buf = Request::parse(buf).expect("Should parse");

    router.add_route(Method::GET, "/api/v1/temperature", |req, res| {
        let lat = req.query("latitude").unwrap_or("0");
        let lon = req.query("longtitude").unwrap_or("0");
        res.body = format!("Lat: {}, Lon: {}", lat, lon).into();
    });

    let handler = router
        .route(&mut req_from_buf)
        .expect("Route should be found");

    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);

    handler(&mut req_from_buf, &mut res);

    // Asserting that both parameters were correctly parsed and injected into the body
    assert_eq!(res.body, "Lat: 1.1, Lon: 2.2".as_bytes());
}

#[test]
fn test_different_methods() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/path", hello_handler);
    router.add_route(Method::POST, "/path", |_, res| {
        res.body = "POST handled".to_string().into();
    });

    let mut req_get = Request {
        method: "GET",
        path: "/path",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    let handler_get = router.route(&mut req_get).unwrap();
    let metadata = ConnectionMetadata { stream: () };
    let mut res_get = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler_get(&mut req_get, &mut res_get);
    assert_eq!(res_get.body, "Hello, World!".as_bytes());

    let mut req_post = Request {
        method: "POST",
        path: "/path",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    let handler_post = router.route(&mut req_post).unwrap();
    let mut res_post = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler_post(&mut req_post, &mut res_post);
    assert_eq!(res_post.body, "POST handled".as_bytes());
}

#[test]
fn test_method_from_str() {
    assert_eq!(Method::from_str("GET"), Ok(Method::GET));
    assert_eq!(Method::from_str("POST"), Ok(Method::POST));
    assert_eq!(Method::from_str("INVALID"), Err(InvalidMethod));
}

#[test]
fn test_method_index() {
    assert_eq!(Method::GET.index(), 0);
    assert_eq!(Method::OPTIONS.index(), 6);
}

#[test]
fn test_route_not_found() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/path", hello_handler);

    let mut req = Request {
        method: "GET",
        path: "/wrong-path",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    assert!(router.route(&mut req).is_none());
}

#[test]
fn test_route_wrong_method() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/path", hello_handler);

    let mut req = Request {
        method: "POST",
        path: "/path",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    assert!(router.route(&mut req).is_none());
}

#[test]
fn test_nested_routes() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/api/v1/user/:name", |req, res| {
        let name = req.param("name").unwrap();
        res.body = format!("User: {}", name).into();
    });

    let mut req = Request {
        method: "GET",
        path: "/api/v1/user/bob",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    let handler = router.route(&mut req).expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req, &mut res);
    assert_eq!(res.body, "User: bob".as_bytes());
}

#[test]
fn test_param_with_multiple_parts() {
    let mut router: Router<()> = Router::new();
    router.add_route(Method::GET, "/a/:b/:c", |req, res| {
        let b = req.param("b").unwrap();
        let c = req.param("c").unwrap();
        res.body = format!("{}/{}/{}", b, c, "end").into();
    });

    let mut req = Request {
        method: "GET",
        path: "/a/foo/bar",
        version: "1.1",
        headers: Vec::new(),
        params: Vec::new(),
        query_params: Vec::new(),
    };
    let handler = router.route(&mut req).expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req, &mut res);
    assert_eq!(res.body, "foo/bar/end".as_bytes());
}

#[test]
fn test_param_with_multiple_parts_and_query_params() {
    let mut router: Router<()> = Router::new();
    let buf = b"GET /api/v1/inc/2?a=1&b=2&c=3 HTTP/1.1\r\n\r\n";
    let mut req_from_buf = Request::parse(buf).expect("Should parse");

    router.add_route(Method::GET, "/api/:version/:operation/:id", |req, res| {
        let id = req.param("id").unwrap();
        let version = req.param("version").unwrap();
        let operation = req.param("operation").unwrap();
        res.body = format!(
            "Version is {}, Operation is {}, ID is {}, query params is {} {} {}",
            version,
            operation,
            id,
            req.query("a").unwrap(),
            req.query("b").unwrap(),
            req.query("c").unwrap(),
        )
        .into();
    });

    let handler = router
        .route(&mut req_from_buf)
        .expect("Route should be found");
    let metadata = ConnectionMetadata { stream: () };
    let mut res = Response::new(&metadata, Status::Ok, "", ContentType::TEXT);
    handler(&mut req_from_buf, &mut res);
    assert_eq!(
        res.body,
        "Version is v1, Operation is inc, ID is 2, query params is 1 2 3".as_bytes()
    );
}
