window.BENCHMARK_DATA = {
  "lastUpdate": 1779658071454,
  "repoUrl": "https://github.com/nexa-net/nexad",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "committer": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "distinct": true,
          "id": "318faab7525b79ba41421272918706421ccc2671",
          "message": "fix: add command field to ContainerConfig, fix runtime integration test",
          "timestamp": "2026-05-23T00:07:22+02:00",
          "tree_id": "bb1d51c1d28bef77f77f760dec6349d8f7c53b7c",
          "url": "https://github.com/nexa-net/nexad/commit/318faab7525b79ba41421272918706421ccc2671"
        },
        "date": 1779488032087,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6904,
            "range": "± 344",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 9210,
            "range": "± 568",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 107962,
            "range": "± 9835",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3315,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4252,
            "range": "± 83",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 202,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 208,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 213,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 232,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 462172,
            "range": "± 24653",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 486140,
            "range": "± 68667",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4361964,
            "range": "± 407881",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "committer": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "distinct": true,
          "id": "464fdb4f460e772a7926329667e3da4824eb7426",
          "message": "style: fix clippy warnings — add Default impl, allow too_many_arguments",
          "timestamp": "2026-05-24T22:36:51+02:00",
          "tree_id": "eaccdc5f91d783b96bd9358203aa3f370b062852",
          "url": "https://github.com/nexa-net/nexad/commit/464fdb4f460e772a7926329667e3da4824eb7426"
        },
        "date": 1779655367171,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6776,
            "range": "± 143",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8232,
            "range": "± 529",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 128932,
            "range": "± 9002",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3242,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4289,
            "range": "± 33",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 211,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 222,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 234,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 228,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 490959,
            "range": "± 43520",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 491475,
            "range": "± 65837",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4458086,
            "range": "± 414648",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "committer": {
            "email": "nassime.abdiou@icloud.com",
            "name": "Nassime Abdiou",
            "username": "na2sime"
          },
          "distinct": true,
          "id": "b8effdb6b56e5a64f179b8ae31b9fa9356aeab5f",
          "message": "style: fix rustfmt for edition 2024 style (import order, line wrapping)",
          "timestamp": "2026-05-24T23:22:14+02:00",
          "tree_id": "e2e2e12040bb56a0625c77d7276af12bebd0f0f7",
          "url": "https://github.com/nexa-net/nexad/commit/b8effdb6b56e5a64f179b8ae31b9fa9356aeab5f"
        },
        "date": 1779658070493,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6712,
            "range": "± 163",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8073,
            "range": "± 489",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 115129,
            "range": "± 10564",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3173,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4201,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 212,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 215,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 222,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 243,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 394243,
            "range": "± 28298",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 495538,
            "range": "± 94691",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4628435,
            "range": "± 529733",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}