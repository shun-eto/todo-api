dev: 
	RUST_LOG=debug cargo watch --exec run --workdir src

test:
	RUST_LOG=debug cargo watch --exec test --workdir src