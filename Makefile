dev: 
	sqlx db create
	sqlx migrate run
	RUST_LOG=debug cargo watch --exec run --workdir src

test:
	sqlx db create
	sqlx migrate run
	RUST_LOG=debug cargo watch --exec test --workdir src

format:
	cargo fmt