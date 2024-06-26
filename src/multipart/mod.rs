use crate::db::dbconn;
use crate::http::token::get_userdata;
use json::JsonValue;
use tokio;
use uuid::Uuid;

pub mod binary;

fn image_name_db(image_name: String) -> String {
    if image_name.is_empty() {
        return image_name;
    }
    return format!("/images/{}", image_name);
}

async fn save(
    title: &str,
    content: &str,
    image_data: Vec<u8>,
    image_name: String,
    email: &str,
) -> String {
    println!(
        "Title: {} Content: {} Image: {} Email: {}",
        title, content, image_name, email
    );

    if !image_name.is_empty() {
        let mut file = tokio::fs::File::create(format!("public/images/{}", image_name))
            .await
            .unwrap();

        // write the image to the file
        tokio::io::AsyncWriteExt::write_all(&mut file, &image_data)
            .await
            .unwrap();
    }

    dbconn()
        .execute(
            "INSERT INTO posts (title, content, image, email) VALUES (?1, ?2, ?3, ?4)",
            &[
                &title,
                &content,
                &image_name_db(image_name).as_str(),
                &email,
            ],
        )
        .unwrap();

    return "HTTP/1.1 301 MOVED PERMANENTLY\r\nLocation: /\r\n\r\n".to_string();
}

pub async fn upload(
    headers: Vec<(String, String)>,
    body: &str,
    complete_buffer: Vec<u8>,
) -> String {
    let sha256_token: &str = match headers
        .iter()
        .find(|header: &&(String, String)| header.0 == "Cookie")
    {
        Some(header) => header
            .1
            .split("token=")
            .collect::<Vec<&str>>()
            .last()
            .unwrap(),
        None => "",
    };
    let decoded: JsonValue = get_userdata(sha256_token).await;

    if sha256_token.is_empty() {
        return "HTTP/1.1 401 UNAUTHORIZED\r\n\r\n".to_string() + "401 UNAUTHORIZED";
    } else {
        if decoded["email"].is_null() {
            return "HTTP/1.1 401 UNAUTHORIZED\r\n\r\n".to_string() + "401 UNAUTHORIZED";
        }
    }

    let content_type: String = headers
        .iter()
        .find(|header: &&(String, String)| header.0 == "Content-Type")
        .unwrap_or(&("Content-Type".to_string(), "text/plain".to_string()))
        .1
        .to_string();

    let boundary: &str = content_type
        .split("boundary=")
        .collect::<Vec<&str>>()
        .last()
        .unwrap();
    let mut parts: Vec<&str> = body.split(boundary).collect();
    parts.remove(0);
    parts.pop();

    let mut title: String = "".to_string();
    let mut content: String = "".to_string();
    let mut image_name: String = String::new();
    let mut image_data: Vec<u8> = Vec::new();

    for part in parts {
        let lines: std::str::Lines = part.lines();

        let mut lines: Vec<&str> = lines.collect();
        lines.remove(0);
        lines.pop();

        let mut part_headers: Vec<(String, String)> = Vec::new();

        for line in &mut lines {
            let mut parts: Vec<&str> = line.split(": ").collect();
            let key: String = parts.remove(0).to_string();
            let value: String = parts.join(": ").to_string();
            if key.is_empty() && value.is_empty() {
                continue;
            } else if !key.is_empty() && value.is_empty() {
                break;
            }
            part_headers.push((key, value));
        }

        println!("PART Headers: {:?}", part_headers);

        let content_disposition: &str = part_headers
            .iter()
            .find(|header: &&(String, String)| header.0 == "Content-Disposition")
            .unwrap()
            .1
            .as_str();

        let mut name: String = content_disposition
            .split(" name=")
            .collect::<Vec<&str>>()
            .last()
            .unwrap()
            .replace("\"", "");

        name = name
            .split(";")
            .collect::<Vec<&str>>()
            .first()
            .unwrap()
            .to_string();
        let name: &str = name.as_str();

        let body_lines: Vec<&str> = lines
            [lines.iter().position(|line| line.is_empty()).unwrap() + 1..]
            .iter()
            .map(|line| line.trim())
            .collect();

        match name {
            "title" => {
                title = body_lines.join(" ");
            }
            "content" => {
                content = urlencoding::encode(body_lines.join("\n").as_str()).to_string();
            }
            "image" => {
                image_name = content_disposition
                    .split("filename=")
                    .collect::<Vec<&str>>()
                    .last()
                    .unwrap()
                    .replace("\"", "");
                if image_name.is_empty() {
                    continue;
                }
                image_name = format!(
                    "asset-{}.{}",
                    Uuid::new_v4(),
                    image_name.split('.').collect::<Vec<&str>>().last().unwrap()
                );
                image_data = binary::find_binary(complete_buffer.clone(), boundary.to_string());
            }
            _ => {}
        }
    }

    let email: &str = decoded["email"].as_str().unwrap();
    let response: String = save(
        title.as_str(),
        content.as_str(),
        image_data,
        image_name,
        email,
    )
    .await;
    return response;
}
