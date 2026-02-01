# Tag Sync

## Dry run (write to file)

```shell
TOKENOVERFLOW_ENV='local' \
  TOKENOVERFLOW_DATABASE_PASSWORD='xxx' \
  cargo run -p so_tag_sync -- --full --dry-rune
```

## Sync from file

```shell
TOKENOVERFLOW_ENV='local' \
  TOKENOVERFLOW_DATABASE_PASSWORD='xxx' \
  cargo run -p so_tag_sync -- --from-file
```
