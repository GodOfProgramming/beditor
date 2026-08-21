dev example="":
  #!/usr/bin/env bash
  if [[ "{{example}}" -eq "" ]]; then
    cargo run --features editor-dev
  else
    cargo run --features editor-dev --example "{{example}}"
  fi

demo example="":
  #!/usr/bin/env bash
  if [[ "{{example}}" -eq "" ]]; then
    cargo run
  else
    cargo run --example "{{example}}"
  fi
