# Uses `jq` to take the `indices_stats.json` and rename each `.indices["<index>"]` key to be the value of `.indices["<index>"].uuid`
#
# {
#   "indices": {
# {
#   "_shards": {
#     "total": 10,
#     "successful": 10,
#     "failed": 0
#       },
#         ".ds-xxxx": {
#           "uuid": "aZ2HhU0TQYSEOUhwCBBb9Q",
#           "health": "green",
#           "status": "open",
#           "primaries": {…},
#           "total": {…},
#           "shards": {…}
#        },
#        ".ds-6VWQ5lQMRjWdxLR6-gCaTg": {
#          "uuid": "aZ2HhU0TQYSEOUhwCBBb9Q",
#          "health": "green",
#          "status": "open",
#          "primaries": {…},
#          "total": {…},
#          "shards": {…}
#       },
#       ...
#   }
# }

function rename_index_stats_to_uuid() {
  jq '. | .indices = (.indices | with_entries(.key=.value.uuid))' indices_stats.json > indices_stats_by_uuid.json
}


# Use `jq` on the input file `data_stream.json`
# for each entry in the `data_streams` array, if the name is "xxxx", rename it to "datastream_\(.key)"
#
# {
#  "data_streams": [
#    {
#      "name": ".kibana-event-log-ds",
#      "timestamp_field": {"name": "@timestamp"},
#      "indices": [{…}, {…}, {…}, {…}, {…}, {…}, {…}, {…}, {…}, {…}, {…}, {…}],
#       "generation": 92,
#       "_meta": {"description": "index template for the Kibana event log", "managed": true},
#       "status": "GREEN",
#       "template": ".kibana-event-log-template",
#       "lifecycle": {"enabled": true, "data_retention": "90d"},
#       "next_generation_managed_by": "Data stream lifecycle",
#       "prefer_ilm": true,
#       "hidden": true,
#       "system": false,
#       "allow_custom_routing": false,
#       "replicated": false,
#       "rollover_on_write": false
#    },
#    ...
#  ]
#}

# {
#     "meta": true,
#     "data_streams": [
#         {
#             "name": "xxxx",
#             "indices": [
#                 { "index_name": "xxxx", "index_uuid": "abcd"}
#             ]
#         },
#         {
#             "name": "xxxx",
#             "indices": [
#                 { "index_name": "xxxx", "index_uuid": "efgh"}
#             ]
#         },
#         {
#             "name": ".kibana",
#             "indices": [
#                 { "index_name": ".kibana-1", "index_uuid": "ijkl"}
#             ]
#         }
#     ]
# }
#
#

function rename_data_stream_indices_to_uuid() {
  jq '. | .data_streams = (.data_streams | to_entries | map(
    if .value.name == "xxxx" then
      .value.name = "data_stream-\(.key)" |
      .value.indices = (.value.indices | map(. + {
        index_name: .index_uuid
      }))
    else . end |
    .value
  ))' data_stream.json > data_stream_renamed.json
}

# Use `jq` on the input file `settings.json`
# For each entry in the input object, if the .key is ".ds-xxxx", rename `.key` to `.settings.index.uuid`
#
# {
#  ".ds-xxxx": {
#     "settings": {
#       "index": {
#         "refresh_interval": "60s",
#         "hidden": "true",
#         "translog": {},
#         "provided_name": ".ds-xxxx",
#         "max_result_window": "100000",
#         "creation_date": "1744085887458",
#         "analysis":{},
#         "number_of_replicas": "1",
#         "uuid": "LANHS_1234567890",
#         "version": {},
#         "lifecycle": {},
#         "routing": {},
#         "number_of_shards": "1"
#       }
#     }
#   },
#   ".ds-xxxx": {
#      "settings": {
#        "index": {
#          "refresh_interval": "60s",
#          "hidden": "true",
#          "translog": {},
#          "provided_name": ".ds-xxxx",
#          "max_result_window": "100000",
#          "creation_date": "1744085887999",
#          "analysis":{},
#          "number_of_replicas": "1",
#          "uuid": "LANHS_ABCDEF0123456789",
#          "version": {},
#          "lifecycle": {},
#          "routing": {},
#          "number_of_shards": "5"
#        }
#      }
#    },
#   ...
# }

# Use `jq` on the input file `settings.json`
# For each entry in the input object, if the .key is ".ds-xxxx", rename `.key` to `.settings.index.uuid`
function rename_index_settings_to_uuid() {
  jq 'with_entries(
    if (.key | startswith(".index-")) then
      .key = .value.settings.index.uuid
    else . end
  )' settings.json > settings_by_uuid.json
}

function renumber_index_names() {
    perl -pe 's/\"\.ds-xxxx\"/sprintf("\".index-%04d\"",++$i)/ge' ${1} > "${1}_numbered.json"
}

# rename_data_stream_indices_to_uuid
# rename_index_stats_to_uuid
rename_index_settings_to_uuid
