#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

#[macro_use] extern crate log;
extern crate simple_logger;
extern crate rocket;
extern crate rocket_cors;
extern crate sqlite;

use std::sync::Mutex;
use rocket::{Rocket, State};
use rocket::http::Method;
use rocket_cors::{AllowedOrigins, AllowedHeaders};
use sqlite::{Connection, Error};
use log::LogLevel;
use rocket::config::{Config, Environment};


type DbConn = Mutex<Connection>;

struct WebserverConfig {
    host: String,
    port: u16
}

fn init_database(db_conn: &Connection, webserver_config: &mut WebserverConfig) {
    if db_conn.execute("SELECT * FROM devices").is_ok() {
        info!("DB: Table 'devices' exists");
    } else {
        warn!("DB: Table 'devices' does not exist. Creating with defautl values.");
        db_conn.execute("CREATE TABLE devices (name TEXT, error BOOL);").unwrap();
    }

    if db_conn.execute("SELECT * FROM config").is_ok() {
        info!("DB: Table 'config' exists");
    } else {
        warn!("DB: Table 'config' does not exist. Creating.");
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

fn get_config_value(db_conn: State<DbConn>, key: &'static str) -> String {
    let mut return_value = "".to_string();
    let sql_statement = "SELECT value FROM config WHERE key = '".to_owned() + key + "'"; // maybe add LIMIT 1
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
fn rocket_devices(db_conn: State<DbConn>) -> &'static str  {
    r#"{
        "devices": [{
            "name": "Hardcoded Device One",
            "error": 0,
            "id": 12
        }, {
            "name": "Hardcoded Device Two",
            "error": 1,
            "id": "eggs"
        }, {
            "name": "Hardcoded Device Three",
            "error": 0,
            "id": "474caade-1cd1-4450-9dd5-1962c37e5206"
        }]
    }"#
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
        .mount("/", routes![rocket_index, rocket_version, rocket_devices])
}

fn main() {
    rocket().launch();
}