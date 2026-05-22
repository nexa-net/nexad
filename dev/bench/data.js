window.BENCHMARK_DATA = {
  "lastUpdate": 1779488033307,
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
      }
    ]
  }
}