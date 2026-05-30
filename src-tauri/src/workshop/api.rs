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
    let response = run_powershell_json_request(&script, "consultar os detalhes dos mods")?;
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
            "Informe um Workshop ID numerico para a {item_label}."
        ));
    }

    Ok(value.to_string())
}

pub(super) fn fetch_steam_workshop_collection_items(
    collection_id: &str,
) -> Result<Vec<String>, String> {
    let collection_id = validate_workshop_id(collection_id, "colecao")?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $body = @{{ collectioncount = '1'; 'publishedfileids[0]' = '{collection_id}' }}; \
         $response = Invoke-RestMethod -Method Post \
           -Uri 'https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/' \
           -Body $body; \
         $response | ConvertTo-Json -Depth 8 -Compress"
    );
    let response = run_powershell_json_request(&script, "consultar a colecao na Steam")?;
    let children = response
        .get("response")
        .and_then(|value| value.get("collectiondetails"))
        .and_then(Value::as_array)
        .and_then(|collections| collections.first())
        .and_then(|collection| collection.get("children"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "A Steam nao encontrou itens nessa colecao. Confirme se o ID pertence a uma colecao publica."
                .to_string()
        })?;
    let mut seen = HashSet::new();
    let workshop_ids = children
        .iter()
        .filter_map(|child| child.get("publishedfileid").and_then(Value::as_str))
        .filter(|workshop_id| workshop_id.chars().all(|char| char.is_ascii_digit()))
        .filter(|workshop_id| seen.insert((*workshop_id).to_string()))
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if workshop_ids.is_empty() {
        return Err("A colecao informada nao possui itens para baixar.".to_string());
    }

    Ok(workshop_ids)
}

fn run_powershell_json_request(script: &str, action: &str) -> Result<Value, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|error| format!("Nao foi possivel {action}: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let details = stderr.trim();
        return Err(if details.is_empty() {
            format!("Nao foi possivel {action}.")
        } else {
            format!("Nao foi possivel {action}:\n{details}")
        });
    }

    serde_json::from_str(&stdout).map_err(|error| {
        format!("A Steam retornou uma resposta invalida ao tentar {action}: {error}")
    })
}
