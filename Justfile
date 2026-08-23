dev example="":
  #!/usr/bin/env bash
  if [[ "{{example}}" == "" ]]; then
    cargo run --features editor-dev
  else
    echo "Running example {{example}}"
    cargo run --features editor-dev --example "{{example}}"
  fi

demo example="":
  #!/usr/bin/env bash
  if [[ "{{example}}" == "" ]]; then
    cargo run
  else
    cargo run --example "{{example}}"
  fi
