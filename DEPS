{
  "version": "1.3.2",
  "vars": {
    "GIT_DOMAIN": "github.com",
    "GITHUB_DOMAIN": "https://${GIT_DOMAIN}"
  },
  "repos": {
    "common": [
      {
        "url": "${GITHUB_DOMAIN}/domchen/depsync.git",
        "commit": "bdb1b059bec551c045a608953d109b5809804383",
        "dir": "third_party/depsync"
      }
    ]
  },
  "files": {
    "common": [
      {
        "url": "https://github.com/0x1306a94/wcdb-spm-prebuilt/releases/download/storage.v2.1.15/WCDBSwift.xcframework.zip",
        "dir": "third_party/WCDBSwift",
        "unzip": true
      }
    ]
  },
  "linkfiles": {
    "common": [
      {
        "src": "third_party/depsync",
        "dest": "reference/depsync"
      },
      {
        "src": "third_party/depsync/third_party/tgfx",
        "dest": "reference/tgfx"
      }
    ]
  },
  "copyfiles": {
    "common": [
      {
        "src": "third_party/WCDBSwift/WCDBSwift.xcframework",
        "dest": "reference/WCDBSwift/WCDBSwift.xcframework"
      }
    ]
  },
  "actions": {
    "common": [
      {
        "command": "depctl --clean",
        "dir": "third_party"
      }
    ]
  }
}