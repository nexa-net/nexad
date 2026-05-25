window.BENCHMARK_DATA = {
  "lastUpdate": 1779720943947,
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
          "id": "21f49aa07d36870c407a76023abab0c92f6c7c5b",
          "message": "ci: add Cross.toml to install protoc for aarch64-linux cross builds",
          "timestamp": "2026-05-25T01:15:42+02:00",
          "tree_id": "0f7279589f0da267fd01b2d186d8d77267996ac8",
          "url": "https://github.com/nexa-net/nexad/commit/21f49aa07d36870c407a76023abab0c92f6c7c5b"
        },
        "date": 1779664792413,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6558,
            "range": "± 145",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8096,
            "range": "± 606",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 113725,
            "range": "± 11053",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3165,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4198,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 214,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 216,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 228,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 242,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 392177,
            "range": "± 98209",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 507893,
            "range": "± 55724",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4499114,
            "range": "± 537420",
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
          "id": "92ef1ab4e017cb720d23e513bede9260ef3a3d4d",
          "message": "ci: fix release workflow — fail-fast false, pre-build protoc for cross",
          "timestamp": "2026-05-25T01:25:36+02:00",
          "tree_id": "b25f5d75be4ddb54359d3e96f64ccc99566974af",
          "url": "https://github.com/nexa-net/nexad/commit/92ef1ab4e017cb720d23e513bede9260ef3a3d4d"
        },
        "date": 1779665404320,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6945,
            "range": "± 360",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8341,
            "range": "± 499",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 107912,
            "range": "± 8858",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3331,
            "range": "± 41",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4277,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 211,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 215,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 228,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 228,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 506361,
            "range": "± 35956",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 486760,
            "range": "± 60601",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4451252,
            "range": "± 453111",
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
          "id": "1157979bfa76e3612718e37b21e3d4515695b4f8",
          "message": "fix: default data-dir to ~/.nexa/data instead of /var/lib/nexa\n\nAvoids permission errors when running without sudo.",
          "timestamp": "2026-05-25T11:53:58+02:00",
          "tree_id": "6002b14ded8e2d93566aa9142dce08ecf4efbe2a",
          "url": "https://github.com/nexa-net/nexad/commit/1157979bfa76e3612718e37b21e3d4515695b4f8"
        },
        "date": 1779703123068,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6608,
            "range": "± 102",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 7899,
            "range": "± 511",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 115720,
            "range": "± 10811",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3131,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4175,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 213,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 217,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 227,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 241,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 372098,
            "range": "± 11062",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 502171,
            "range": "± 55845",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4784703,
            "range": "± 481522",
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
          "id": "6e6fe9d123c065a403311ff5a08384e3eb7e2273",
          "message": "style(nexad): cargo fmt",
          "timestamp": "2026-05-25T16:24:31+02:00",
          "tree_id": "962bbb9fae57f99daddcd8d061df8cbf8c23e9f9",
          "url": "https://github.com/nexa-net/nexad/commit/6e6fe9d123c065a403311ff5a08384e3eb7e2273"
        },
        "date": 1779719980366,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 7035,
            "range": "± 96",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8478,
            "range": "± 553",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 107552,
            "range": "± 9330",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3329,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4303,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 201,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 214,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 217,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 217,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 497474,
            "range": "± 37109",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 478737,
            "range": "± 63722",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4417697,
            "range": "± 439455",
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
          "id": "c47981dea85e0101113b9c62708bf84d2fa80980",
          "message": "release: bump to v0.2.0 and add auto-release workflow",
          "timestamp": "2026-05-25T16:44:37+02:00",
          "tree_id": "72afeb613f41c08116c1ecc0426835f29e11b89e",
          "url": "https://github.com/nexa-net/nexad/commit/c47981dea85e0101113b9c62708bf84d2fa80980"
        },
        "date": 1779720769198,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 5109,
            "range": "± 127",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 6859,
            "range": "± 395",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 90429,
            "range": "± 7838",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 2464,
            "range": "± 96",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 3266,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 157,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 159,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 166,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 183,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 475878,
            "range": "± 1308474",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 418823,
            "range": "± 46800",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 3427068,
            "range": "± 364474",
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
          "id": "2a79d3d6a531fbf183be651e8103bba2eeafe8df",
          "message": "ci: auto-release builds and publishes directly on version bump",
          "timestamp": "2026-05-25T16:51:27+02:00",
          "tree_id": "338969e7d89922dc3ff73d3bd91441142e261938",
          "url": "https://github.com/nexa-net/nexad/commit/2a79d3d6a531fbf183be651e8103bba2eeafe8df"
        },
        "date": 1779720942981,
        "tool": "cargo",
        "benches": [
          {
            "name": "encrypt/64B",
            "value": 6859,
            "range": "± 101",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/1KB",
            "value": 8360,
            "range": "± 550",
            "unit": "ns/iter"
          },
          {
            "name": "encrypt/64KB",
            "value": 162406,
            "range": "± 10460",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/64B",
            "value": 3158,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "decrypt/1KB",
            "value": 4217,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/10",
            "value": 201,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/100",
            "value": 208,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "dns_lookup/records/1000",
            "value": 214,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_register_deregister",
            "value": 248,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "insert_pod",
            "value": 384763,
            "range": "± 12384",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/100",
            "value": 511872,
            "range": "± 53254",
            "unit": "ns/iter"
          },
          {
            "name": "list_pods/1000",
            "value": 4242943,
            "range": "± 489056",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}