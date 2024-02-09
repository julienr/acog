.PHONY: display json display_small json_small

all: display_small

display:
	cargo run --bin to_npy -- example_data/local/marina_cog_nocompress.tif && python utils/npyshow.py img.npy

display_small:
	cargo run --bin to_npy -- example_data/example_1_cog_nocompress.tif && python utils/npyshow.py img.npy

json:
	cargo run -F json --bin to_json -- example_data/local/marina_cog_nocompress.tif && jq . out.json

json_small:
	cargo run -F json --bin to_json -- example_data/example_1_cog_nocompress.tif && jq . out.json

clippy:
	cargo clippy --all-features
