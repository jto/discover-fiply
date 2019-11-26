use serde::Deserialize;
use serde_json::json;
use serde_json::value::Value;
use std::time::SystemTime;

#[derive(Debug)]
pub enum FipClientError {
  JsonError(serde_json::error::Error),
  WeirdFipJsonError,
  FipError(reqwest::Error),
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct TimelineItem {
  pub album: String,
  pub subtitle: String,
  pub interpreters: Vec<String>,
  pub year: Option<u16>,
  pub start_time: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TimeLineItemEdge {
  node: TimelineItem,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct PageInfo {
  #[serde(rename = "endCursor")]
  pub end_cursor: String,
  #[serde(rename = "hasNextPage")]
  pub has_next_page: bool,
}

const FIP_URI: &'static str = "https://www.fip.fr/latest/api/graphql";

fn go_down(value: &Value) -> Option<(Value, Value)> {
  let root = value
    .as_object()?
    .get("data")?
    .as_object()?
    .get("timelineCursor")?
    .as_object()?;

  let edges = root.get("edges")?;
  let page_info = root.get("pageInfo")?;

  Some(((*edges).clone(), (*page_info).clone()))
}

// TODO: proper error
fn parse_songs(value: Value) -> Result<(Vec<TimelineItem>, PageInfo), FipClientError> {
  let (edges, info) = go_down(&value).map_or(Err(FipClientError::WeirdFipJsonError), |v| Ok(v))?;
  let mut es: Vec<TimeLineItemEdge> = vec![];
  let edges_iter: Vec<&Value> = edges
    .as_array()
    .iter()
    .flat_map(|vs| vs.into_iter())
    .collect();
  for a in edges_iter {
    let item = serde_json::from_value(a.clone());
    match item {
      Ok(i) => es.push(i),
      Err(e) =>
      // ignore invalid nodes
      {
        log::warn!(
          "Got error {:?} while parsing edge:\n {}",
          e,
          serde_json::to_string_pretty(&a).unwrap()
        )
      }
    }
  }

  let page_info: PageInfo = serde_json::from_value(info).map_err(|e| {
    log::error!("Could not parse page_info. {:?}", e);
    FipClientError::JsonError(e)
  })?;
  Ok((es.into_iter().map(|e| e.node).collect(), page_info))
}

/// Create the query parameters
fn build_query(time: SystemTime) -> reqwest::RequestBuilder {
  let time_sec: u64 = time
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();

  let time_encoded = base64::encode(&time_sec.to_string());

  let variables = json!({
      "first": 100,
      "after": time_encoded,
      "stationId": 7,
  })
  .to_string();

  let extensions = json!({
      "persistedQuery": {
          "version":1,
          "sha256Hash":"ce6791c62408f27b9338f58c2a4b6fdfd9d1afc992ebae874063f714784d4129"
      }
  })
  .to_string();

  let params = [
    ("operationName", "History"),
    ("variables", &variables),
    ("extensions", &extensions),
  ];

  // TODO: could use be async
  let client = reqwest::Client::new();
  client.get(FIP_URI).query(&params)
}

///
/// Fetch the songs played _before_ the given time using the FIP API.
/// Note: the API call was reversed engineered from their website.
///
// TODO: return proper error
// TODO: check status code
pub fn fetch_songs(time: SystemTime) -> Result<(Vec<TimelineItem>, PageInfo), FipClientError> {
  log::info!("fethcing song at date {:?}", time);

  let resp = build_query(time)
    .send()
    .map_err(|e| {
      log::error!("Got error {:?} from Http client", e);
      FipClientError::FipError(e)
    })?
    .json()
    .map(|j| {
      log::debug!(
        "FIP Json body: {}",
        serde_json::to_string_pretty(&j).unwrap()
      );
      j
    })
    .map_err(|e| {
      log::error!("Got error {:?} while getting Json body", e);
      FipClientError::FipError(e)
    })?;
  parse_songs(resp)
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::value::Value;

  fn json_timeline_item() -> Value {
    json!({
        "__typename": "TimeLineItemEdge",
        "node": {
            "__typename": "TimelineItem",
            "subtitle": "Cheney Lane",
            "start_time": 1572251703,
            "end_time": 1572251846,
            "cover": "https://cdn.radiofrance.fr/s3/cruiser-production/2018/05/6a057c58-de15-485b-b03e-92b61467f18a/400x400_rf_omm_0001646155_dnc.0076882786.jpg",
            "label": "KITSUNE",
            "album": "Café Kitsuné mix",
            "interpreters": [
            "Nostalgia 77"
            ],
            "musical_kind": "Musique électronique ",
            "year": 2018,
            "external_links": {
            "__typename": "ExternalLinks",
            "youtube": {
                "__typename": "ExternalLink",
                "link": "https://www.youtube.com/watch?v=C20jcpj3Q0w",
                "image": "https://i.ytimg.com/vi/C20jcpj3Q0w/hqdefault.jpg"
            }
            },
            "title": "Nostalgia 77"
        },
        "cursor": "MTU3MjI1MTg0Ng=="
    })
  }

  fn expected_item() -> TimelineItem {
    TimelineItem {
      album: String::from("Café Kitsuné mix"),
      subtitle: String::from("Cheney Lane"),
      interpreters: vec![String::from("Nostalgia 77")],
      year: Some(2018),
      start_time: 1572251703,
    }
  }

  fn json_timeline_items() -> Value {
    json!({
      "data": {
        "timelineCursor": {
          "__typename": "HistoryCursor",
          "totalCount": 0,
          "edges": [json_timeline_item()],
          "pageInfo": {
            "__typename": "PageInfo",
            "endCursor": "MTU3NDY4OTI4Mg==",
            "hasNextPage": true
          },
        }
      }
    })
  }

  #[test]
  fn test_parse_songs() {
    let json_items = json_timeline_items();
    let expected = (
      vec![expected_item()],
      PageInfo {
        end_cursor: String::from("MTU3NDY4OTI4Mg=="),
        has_next_page: true,
      },
    );
    let result = parse_songs(json_items);

    assert!(
      result.is_ok(),
      "Expected an Ok(...) result but got en error"
    );
    assert_eq!(result.unwrap(), expected);
  }
}
