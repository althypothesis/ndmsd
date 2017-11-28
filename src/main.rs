#[macro_use] extern crate log;
extern crate simple_logger;
extern crate hyper;
extern crate futures;
extern crate sqlite;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use futures::future::Future;
use hyper::header::AccessControlAllowOrigin;
use hyper::server::{Http, Request, Response, Service};
use hyper::{Method, StatusCode};
use log::LogLevel;

fn get_config_value(c: &sqlite::Connection, key: &'static str) -> String {
	let mut return_value = "".to_string();
	let sql_statement = "SELECT value FROM config WHERE key = '".to_owned() + key + "'"; // maybe add LIMIT 1
	c.iterate(sql_statement, |values| {
		for &(column, value) in values.iter() {
			return_value = value.unwrap().to_string();
			debug!("get_config_value(): {} = {}", column, return_value);
			break;
		}
		true
	}).unwrap();
	return_value
}

fn main() {
	simple_logger::init_with_level(LogLevel::Info).unwrap();

	info!("Starting ndmsd {}...", env!("CARGO_PKG_VERSION"));

	struct WebService;

	impl Service for WebService {
		// boilerplate hooking up hyper's server types
		type Request = Request;
		type Response = Response;
		type Error = hyper::Error;
		// The future representing the eventual Response your call will
		// resolve to. This can change to whatever Future you need.
		type Future = Box<Future<Item=Self::Response, Error=Self::Error>>;

		fn call(&self, req: Request) -> Self::Future {
			let mut response = Response::new().with_header(AccessControlAllowOrigin::Any);

			 match (req.method(), req.path()) {
				(&Method::Get, "/") => {
					response.set_body("ndmsd is running");
				},
				(&Method::Get, "/version") => {
					response.set_body("{\"version\": \"".to_owned() + env!("CARGO_PKG_VERSION") + "\"}");
				},
				(&Method::Post, "/echo") => {
					response.set_body(req.body());
				},
				_ => {
					response.set_status(StatusCode::NotFound);
				},
			};

			Box::new(futures::future::ok(response))
		}
	}

	let db = sqlite::open("./ndmsd_db.sqlite3").unwrap();

	if db.execute("SELECT * FROM devices").is_ok() {
		info!("DB: Table 'devices' exists");
	} else {
		warn!("DB: Table 'devices' does not exist. Creating.");
		db.execute("CREATE TABLE devices (name TEXT, services TEXT);").unwrap();
	}

	if db.execute("SELECT * FROM config").is_ok() {
		info!("DB: Table 'config' exists");
	} else {
		warn!("DB: Table 'config' does not exist. Creating.");
		db.execute("
			CREATE TABLE config (key TEXT, value TEXT);
			INSERT INTO config (key, value) VALUES ('web_port','12526');
			INSERT INTO config (key, value) VALUES ('web_host','127.0.0.1');
		").unwrap();
	}

	/*db.iterate("SELECT * FROM devices WHERE age > 40", |pairs| {
		for &(column, value) in pairs.iter() {
			println!("{} = {}", column, value.unwrap());
		}
		true
	})
	.unwrap();*/
	/*
	let web_port = db.iterate("SELECT * FROM config WHERE key = 'web_port'", |port| {
		for &(column, value) in pairs.iter() {
			println!("{} = {}", column, value.unwrap());
		}
		true
	});*/
	/*match web_port {
		Ok(v) => println!("DB: web_port: {:?}", v.Value),
		Err(e) => println!("DB: Error getting web_port")
	}*/

	let web_bind_address = get_config_value(&db, "web_host") + ":" + &get_config_value(&db, "web_port");

	info!("Starting webserver on {}...", web_bind_address);

	let server = Http::new().bind(&web_bind_address.parse().unwrap(), || Ok(WebService)).unwrap();
	server.run().unwrap();
}