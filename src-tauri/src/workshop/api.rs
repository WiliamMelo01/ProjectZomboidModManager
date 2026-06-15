use crate::i18n::text;
use crate::util::hide_command_window;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    process::Command,
};

pub(super) fn fetch_steam_workshop_item_names(
    workshop_ids: &[String],
) -> Result<HashMap<String, String>, String> {
    if workshop_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut body = format!("itemcount = '{}'; ", workshop_ids.len());

    for (index, workshop_id) in workshop_ids.iter().enumerate() {
        let workshop_id = validate_workshop_id(workshop_id, "item")?;
        body.push_str(&format!("'publishedfileids[{index}]' = '{workshop_id}'; "));
    }

    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $body = @{{ {body} }}; \
         $response = Invoke-RestMethod -Method Post \
           -Uri 'https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/' \
           -Body $body; \
         $response | ConvertTo-Json -Depth 8 -Compress"
    );
    let response = run_powershell_json_request(
        &script,
        &text("fetch mod details", "consultar os detalhes dos mods"),
    )?;
    let mut names = HashMap::new();

    if let Some(items) = response
        .get("response")
        .and_then(|value| value.get("publishedfiledetails"))
        .and_then(Value::as_array)
    {
        for item in items {
            if let (Some(workshop_id), Some(name)) = (
                item.get("publishedfileid").and_then(Value::as_str),
                item.get("title").and_then(Value::as_str),
            ) {
                names.insert(workshop_id.to_string(), name.to_string());
            }
        }
    }

    Ok(names)
}

pub(super) fn validate_workshop_id(value: &str, item_label: &str) -> Result<String, String> {
    let value = value.trim();

    if value.is_empty() || !value.chars().all(|char| char.is_ascii_digit()) {
        return Err(format!(
            "{} {}.",
            text(
                "Enter a numeric Workshop ID for",
                "Informe um Workshop ID numerico para"
            ),
            item_label
        ));
    }

    Ok(value.to_string())
}

pub(super) fn fetch_steam_workshop_collection_items(
    collection_id: &str,
) -> Result<Vec<String>, String> {
    let collection_id = validate_workshop_id(collection_id, &text("collection", "colecao"))?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $body = @{{ collectioncount = '1'; 'publishedfileids[0]' = '{collection_id}' }}; \
         $response = Invoke-RestMethod -Method Post \
           -Uri 'https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/' \
           -Body $body; \
         $response | ConvertTo-Json -Depth 8 -Compress"
    );
    let response = run_powershell_json_request(
        &script,
        &text("fetch the Steam collection", "consultar a colecao na Steam"),
    )?;

    let mut workshop_ids = collection_item_ids_from_api_response(&response);

    if workshop_ids.is_empty() {
        workshop_ids = fetch_steam_workshop_collection_items_from_page(&collection_id)?;
    }

    if workshop_ids.is_empty() {
        return Err(text(
            "Steam did not find items in this collection. Confirm that the ID belongs to a public collection.",
            "A Steam nao encontrou itens nessa colecao. Confirme se o ID pertence a uma colecao publica.",
        )
        .to_string());
    }

    Ok(workshop_ids)
}

fn collection_item_ids_from_api_response(response: &Value) -> Vec<String> {
    let Some(children) = response
        .get("response")
        .and_then(|value| value.get("collectiondetails"))
        .and_then(Value::as_array)
        .and_then(|collections| collections.first())
        .and_then(|collection| collection.get("children"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let mut seen = HashSet::new();

    children
        .iter()
        .filter_map(|child| child.get("publishedfileid").and_then(Value::as_str))
        .filter(|workshop_id| workshop_id.chars().all(|char| char.is_ascii_digit()))
        .filter(|workshop_id| seen.insert((*workshop_id).to_string()))
        .map(ToString::to_string)
        .collect()
}

fn fetch_steam_workshop_collection_items_from_page(
    collection_id: &str,
) -> Result<Vec<String>, String> {
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $response = Invoke-WebRequest \
           -UseBasicParsing \
           -Uri 'https://steamcommunity.com/sharedfiles/filedetails/?id={collection_id}'; \
         $response.Content"
    );
    let html = run_powershell_text_request(
        &script,
        &text("fetch the Steam collection page", "consultar a pagina da colecao na Steam"),
    )?;

    Ok(collection_item_ids_from_html(&html, collection_id))
}

fn collection_item_ids_from_html(html: &str, collection_id: &str) -> Vec<String> {
    let html = html
        .find("collectionItem")
        .and_then(|index| html.get(index..))
        .unwrap_or(html);
    let mut seen = HashSet::new();
    let mut workshop_ids = Vec::new();

    for pattern in [
        "sharedfiles/filedetails/?id=",
        "workshop/filedetails/?id=",
    ] {
        let mut remaining = html;

        while let Some(index) = remaining.find(pattern) {
            remaining = &remaining[index + pattern.len()..];
            let workshop_id = remaining
                .chars()
                .take_while(|char| char.is_ascii_digit())
                .collect::<String>();

            if workshop_id.is_empty() {
                continue;
            }

            if workshop_id != collection_id && seen.insert(workshop_id.clone()) {
                workshop_ids.push(workshop_id);
            }
        }
    }

    workshop_ids
}

fn run_powershell_json_request(script: &str, action: &str) -> Result<Value, String> {
    let mut command = Command::new("powershell.exe");
    let output = hide_command_window(&mut command)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|error| {
            format!(
                "{} {action}: {error}",
                text("Could not", "Nao foi possivel")
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let details = stderr.trim();
        return Err(if details.is_empty() {
            format!("{} {action}.", text("Could not", "Nao foi possivel"))
        } else {
            format!(
                "{} {action}:\n{details}",
                text("Could not", "Nao foi possivel")
            )
        });
    }

    serde_json::from_str(&stdout).map_err(|error| {
        format!(
            "{} {action}: {error}",
            text(
                "Steam returned an invalid response while trying to",
                "A Steam retornou uma resposta invalida ao tentar"
            )
        )
    })
}

fn run_powershell_text_request(script: &str, action: &str) -> Result<String, String> {
    let mut command = Command::new("powershell.exe");
    let output = hide_command_window(&mut command)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|error| {
            format!(
                "{} {action}: {error}",
                text("Could not", "Nao foi possivel")
            )
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let details = stderr.trim();
        return Err(if details.is_empty() {
            format!("{} {action}.", text("Could not", "Nao foi possivel"))
        } else {
            format!(
                "{} {action}:\n{details}",
                text("Could not", "Nao foi possivel")
            )
        });
    }

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_collection_items_from_api_response() {
        let response = json!({
            "response": {
                "collectiondetails": [
                    {
                        "children": [
                            { "publishedfileid": "111" },
                            { "publishedfileid": "222" },
                            { "publishedfileid": "111" }
                        ]
                    }
                ]
            }
        });

        assert_eq!(
            collection_item_ids_from_api_response(&response),
            vec!["111".to_string(), "222".to_string()]
        );
    }

    #[test]
    fn reads_collection_items_from_html_fallback() {
        let html = r#"
            <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=3073059898">same collection</a>
            <div class="collectionItem">
              <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=2694448564">Mod Manager</a>
              <a href="https://steamcommunity.com/workshop/filedetails/?id=2725216703">Another mod</a>
              <a href="https://steamcommunity.com/sharedfiles/filedetails/?id=2694448564">duplicate</a>
            </div>
        "#;

        assert_eq!(
            collection_item_ids_from_html(html, "3073059898"),
            vec!["2694448564".to_string(), "2725216703".to_string()]
        );
    }
}
