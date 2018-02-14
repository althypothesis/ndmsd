#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

#[macro_use] extern crate log;
extern crate simple_logger;
extern crate rocket;
extern crate rocket_cors;
extern crate sqlite;
#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::sync::Mutex;
use rocket::{Rocket, State};
use rocket::http::Method;
use rocket_cors::{AllowedOrigins, AllowedHeaders};
use sqlite::{Connection};
use log::LogLevel;
use rocket::config::{Config, Environment};
use rocket::response::status::NotFound;
use std::thread;


type DbConn = Mutex<Connection>;

struct WebserverConfig {
	host: String,
	port: u16
}

#[derive(Serialize, Deserialize, Debug)]
struct Device {
	name: String,
	error: bool,
	id: String
}

#[derive(Serialize, Deserialize, Debug)]
struct Service {
	name: String,
	error: bool,
	id: String
}

#[derive(Serialize, Deserialize, Debug)]
struct DevicesResponse {
	devices: Vec<Device>
}

#[derive(Serialize, Deserialize, Debug)]
struct DeviceResponse {
	device: Device,
	services: Vec<Service>
}

fn init_database(db_conn: &Connection, webserver_config: &mut WebserverConfig) {
	if db_conn.execute("SELECT * FROM devices").is_ok() {
		info!("DB: Table 'devices' exists");
	} else {
		warn!("DB: Table 'devices' does not exist. Creating.");
		db_conn.execute("CREATE TABLE devices (name TEXT, error BOOL, uuid TEXT PRIMARY KEY);").unwrap();
	}

	if db_conn.execute("SELECT * FROM services").is_ok() {
		info!("DB: Table 'services' exists");
	} else {
		warn!("DB: Table 'services' does not exist. Creating.");
		db_conn.execute("create table services(device TEXT, name TEXT, error BOOL, uuid TEXT PRIMARY KEY);").unwrap();
	}

	if db_conn.execute("SELECT * FROM config").is_ok() {
		info!("DB: Table 'config' exists");
	} else {
		warn!("DB: Table 'config' does not exist. Creating with default values.");
		db_conn.execute("
			CREATE TABLE config (key TEXT, value TEXT);
			INSERT INTO config (key, value) VALUES ('web_port','12526');
			INSERT INTO config (key, value) VALUES ('web_host','0.0.0.0');
		").unwrap();
	}

	// Get config for webserver (this needs to be done early, so we do it here)
	db_conn.iterate("SELECT value FROM config WHERE key = 'web_host'", |values| {
		for &(_column, value) in values.iter() {
			webserver_config.host = value.unwrap().to_string();
			debug!("init_databse(): webserver_config.host is: {}", webserver_config.host);
			break;
		}
		true
	}).unwrap();
	db_conn.iterate("SELECT value FROM config WHERE key = 'web_port'", |values| {
		for &(_column, value) in values.iter() {
			webserver_config.port = value.unwrap().parse::<u16>().unwrap();
			debug!("init_databse(): webserver_config.port is: {}", webserver_config.port);
			break;
		}
		true
	}).unwrap();
}

// Not actually using this yet. Future, maybe?
/*
fn get_config_value(db_conn: State<DbConn>, key: &'static str) -> String {
	let mut return_value = "".to_string();
	let sql_statement = "SELECT value FROM config WHERE key = '".to_owned() + key + "' LIMIT 1"; // maybe add LIMIT 1
	db_conn.lock()
		.expect("db connection lock")
		.iterate(sql_statement, |values| {
			for &(column, value) in values.iter() {
				return_value = value.unwrap().to_string();
				debug!("get_config_value(): {} = {}", column, return_value);
				break;
			}
			true
		}).unwrap();
	return_value
}
*/

#[get("/")]
fn rocket_index(_db_conn: State<DbConn>) -> &'static str  {
	"ndmsd is running"
}

#[get("/version")]
fn rocket_version(_db_conn: State<DbConn>) -> &'static str  {
	concat!("{ \"version\": \"",
		env!("CARGO_PKG_VERSION"),
		"\" }")
}

#[get("/devices")]
fn rocket_devices(db_conn: State<DbConn>) -> String  { // previously &'static str
	let mut vec = Vec::new();
	
	// Hard-coded devices for testing
	//vec.push(Device {name:"Device 1".to_string(),error:false,id:"069666e8-28ef-4411-8fa8-1072ebd519f6".to_string()});
	//vec.push(Device {name:"Device 2".to_string(),error:true,id:"16cafb8f-87fa-45ea-a4b9-1f1d79e4c613".to_string()});

	db_conn.lock() // get device info
		.expect("db connection lock")
		.iterate("SELECT * FROM devices", |devices| {
			let mut d = Device { name: "DB Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
			for &(column, value) in devices.iter() {
				//debug!("rocket_devices(): {} = {}", column, value.unwrap());
				if column == "name" {
					d.name = value.unwrap().to_string();
				} else if column == "error" {
					if value.unwrap() == "0" { d.error = false; }
				} else if column == "uuid" {
					d.id = value.unwrap().to_string();
				} else {
					error!("Unknown column: \"{}\"", column);
				}
			}
			vec.push(d);
			//debug!("rocket_devices(): Row: {:?}", devices);
			true
		})
		.unwrap();

	let response_object = DevicesResponse { 
		devices: vec
	};
	serde_json::to_string(&response_object).unwrap()
}

#[get("/device/<id>")]
fn rocket_device(db_conn: State<DbConn>, id: String) -> Result<String, NotFound<String>> { // previously &'static str
	let mut s_vec = Vec::new();
	let mut d = Device { name: "DB Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
	//let mut s = Service { name: "Service Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
	let mut results_returned = false;

	db_conn.lock() // get device info
		.expect("db connection lock")
		.iterate(format!("SELECT * FROM devices WHERE uuid = \"{}\" LIMIT 1;", id), |devices| {
			//let mut d = Device { name: "DB Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
			for &(column, value) in devices.iter() {
				//debug!("rocket_devices(): {} = {}", column, value.unwrap());
				if column == "name" {
					d.name = value.unwrap().to_string();
				} else if column == "error" {
					if value.unwrap() == "0" { d.error = false; }
				} else if column == "uuid" {
					d.id = value.unwrap().to_string();
				} else {
					error!("Non existent device id: {}", id);
				}
			}
			//vec.push(d);
			//debug!("rocket_devices(): Row: {:?}", devices);
			results_returned = true;
			true
		})
		.unwrap();

	db_conn.lock() // get services
		.expect("db connection lock")
		.iterate(format!("SELECT * FROM services WHERE device = \"{}\";", id), |services| {
			//let mut d = Device { name: "DB Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
			let mut s = Service { name: "Service Error".to_string(), error: true, id: "00000000-0000-0000-0000-000000000000".to_string() };
			for &(column, value) in services.iter() {
				//debug!("rocket_devices(): {} = {}", column, value.unwrap());
				if column == "name" {
					s.name = value.unwrap().to_string();
				} else if column == "error" {
					if value.unwrap() == "0" { s.error = false; }
				} else if column == "uuid" {
					s.id = value.unwrap().to_string();
				} else {
					error!("Non existent service id: {}", id);
				}
			}
			s_vec.push(s);
			//debug!("rocket_devices(): Row: {:?}", devices);
			//results_returned = true;
			true
		})
		.unwrap();

	if results_returned {
		let response_object = DeviceResponse { 
			device: d,
			services: s_vec
		};
		let response = serde_json::to_string(&response_object).unwrap();
		Ok(response)
	} else {
		Err(NotFound(format!("{{ \"error\": \"Non existent device id: {}\" }}", id)))
	}
}

fn rocket() -> Rocket {
	simple_logger::init_with_level(LogLevel::Info).unwrap();
	info!("Starting ndmsd {}", env!("CARGO_PKG_VERSION"));

	// Open sqlite database
	info!("Opening databse");
	let conn = sqlite::open("./ndmsd_db.sqlite3").unwrap();

	// Initialize the database and get config values
	let mut webserver_config = WebserverConfig {
		host: "".to_string(),
		port: 0
	};
	init_database(&conn, &mut webserver_config);

	// Have Rocket manage the database pool.
	info!("Starting webserver");
	let config = Config::build(Environment::Staging)
		.address(webserver_config.host)
		.port(webserver_config.port)
		.workers(4)
		.unwrap();

	// CORS
	//let (allowed_origins, failed_origins) = AllowedOrigins::all();
	//assert!(failed_origins.is_empty());
	let options = rocket_cors::Cors { // You can also deserialize this
		allowed_origins: AllowedOrigins::all(),
		allowed_methods: vec![Method::Get].into_iter().map(From::from).collect(),
		allowed_headers: AllowedHeaders::some(&["Authorization", "Accept"]),
		allow_credentials: true,
		..Default::default()
	};

	//rocket::ignite()
	rocket::custom(config, false)
		.manage(Mutex::new(conn))
		.attach(options)
		.mount("/", routes![rocket_index, rocket_version, rocket_devices, rocket_device])
}

fn main() {
	let webserver_thread = thread::spawn(|| { // spin off webserver thread
		rocket().launch();
	});

	let webserver_thread_result = webserver_thread.join();
	
	match webserver_thread_result {
		Ok(k) => debug!("Webserver exited successfully: {:?}", k),
		Err(r) => warn!("Webserver thread did not exit cleanly: {:?}", r)
	}
}