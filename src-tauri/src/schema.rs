diesel::table! {
  download_tasks {
      id -> Integer,
      dl_type -> Text,
      status -> Text,
      local_path -> Text,
      cache_json -> Text,
      url -> Text,
      author -> Text,
      comic_name -> Text,
      progress -> Text,
      done -> Bool,
  }
}
