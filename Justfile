contract-package := "shitcoin-garden"
mock-dex-package := "mock-dex"

contract-dist: contract-build contract-optimize contract-checksum

contract-build:
	RUSTFLAGS="-C link-arg=-s" cargo build \
		--package {{contract-package}} \
		--lib --release --target wasm32-unknown-unknown && \
	RUSTFLAGS="-C link-arg=-s" cargo build \
		--package {{mock-dex-package}} \
		--lib --release --target wasm32-unknown-unknown

clean-contract-artifacts:
	rm -rf artifacts

_mk-artifacts-dir:
	mkdir artifacts

contract-checksum:
	cd artifacts && sha256sum {{contract-package}}.wasm > checksum.txt

contract-optimize: clean-contract-artifacts _mk-artifacts-dir
	wasm-opt -Os --signext-lowering \
		target/wasm32-unknown-unknown/release/{{snakecase(contract-package)}}.wasm \
		-o artifacts/{{contract-package}}.wasm && \
	wasm-opt -Os --signext-lowering \
		target/wasm32-unknown-unknown/release/{{snakecase(mock-dex-package)}}.wasm \
		-o artifacts/{{mock-dex-package}}.wasm

contract-test:
	cargo nextest run --package {{contract-package}}

contract-coverage:
	cargo tarpaulin --packages {{contract-package}}

contract-schema:
	cargo run --package {{contract-package}} --bin schema

contract-deploy:
	bun run scripts/deploy.js

develop-web:
	watchexec \
		--watch web \
		--exts js,css,html,rs \
		--ignore bundle.js \
		--restart \
		-- 'bun build web/js/script.js --outfile web/static/bundle.js && cargo shuttle run --wd web'

localnet-test:
	bun run scripts/localnet-integration.test.js

astroport-test:
	bun run scripts/astroport-integration.test.js
