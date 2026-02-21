use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_cors::Cors;
use serde::{Serialize, Deserialize};
use mongodb::{Client, Collection};
use mongodb::bson::doc;
use futures::stream::StreamExt;

#[derive(Serialize, Deserialize, Clone)]
struct Task {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    title: String,
}

struct AppState {
    tasks_collection: Collection<mongodb::bson::Document>,
}

async fn get_tasks(data: web::Data<AppState>) -> impl Responder {
    let mut cursor = data.tasks_collection.find(doc! {}).await.unwrap();
    let mut tasks = Vec::new();
    
    while let Some(result) = cursor.next().await {
        match result {
            Ok(doc) => {
                if let (Some(id), Some(title)) = (doc.get_object_id("_id").ok(), doc.get_str("title").ok()) {
                    tasks.push(serde_json::json!({
                        "_id": id.to_hex(),
                        "title": title
                    }));
                }
            }
            Err(_) => {}
        }
    }
    
    HttpResponse::Ok().json(tasks)
}

async fn add_task(data: web::Data<AppState>, task: web::Json<Task>) -> impl Responder {
    let new_task = doc! {
        "title": &task.title
    };
    
    match data.tasks_collection.insert_one(new_task).await {
        Ok(_) => HttpResponse::Created().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn update_task(data: web::Data<AppState>, path: web::Path<String>, task: web::Json<Task>) -> impl Responder {
    let id = path.into_inner();
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&id) {
        Ok(oid) => oid,
        Err(_) => return HttpResponse::BadRequest().body("Invalid ID"),
    };
    
    let filter = doc! { "_id": object_id };
    let update = doc! { "$set": { "title": &task.title } };
    
    match data.tasks_collection.update_one(filter, update).await {
        Ok(result) => {
            if result.matched_count > 0 {
                HttpResponse::Ok().finish()
            } else {
                HttpResponse::NotFound().finish()
            }
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn delete_task(data: web::Data<AppState>, path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&id) {
        Ok(oid) => oid,
        Err(_) => return HttpResponse::BadRequest().body("Invalid ID"),
    };
    
    let filter = doc! { "_id": object_id };
    
    match data.tasks_collection.delete_one(filter).await {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok().finish()
            } else {
                HttpResponse::NotFound().finish()
            }
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Welcome to Rust Backend API! Visit /tasks to see all tasks.")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    
    let mongodb_uri = std::env::var("MONGODB_URI").expect("MONGODB_URI must be set in .env file");
    
    let client = Client::with_uri_str(&mongodb_uri)
        .await
        .expect("Failed to connect to MongoDB");
    
    let database = client.database("rust_backend");
    let tasks_collection = database.collection::<mongodb::bson::Document>("tasks");
    
    let app_data = web::Data::new(AppState { tasks_collection });

    let host = "0.0.0.0";
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid number");

    println!("Server running at http://{}:{}/", host, port);
    println!("Connected to MongoDB!");

    HttpServer::new(move || {
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(app_data.clone())
            .route("/", web::get().to(index))
            .route("/tasks", web::get().to(get_tasks))
            .route("/tasks", web::post().to(add_task))
            .route("/tasks/{id}", web::put().to(update_task))
            .route("/tasks/{id}", web::delete().to(delete_task))
    })
    .bind((host, port))?
    .run()
    .await
}