.PHONY: licenses

licenses:
	cargo about generate about.hbs -o RUST_THIRD_PARTY_LICENSES.html -c about.toml
	cd nodejs && cargo about generate ../about.hbs -o RUST_THIRD_PARTY_LICENSES.html -c ../about.toml
	cd nodejs && npx license-checker --markdown --out NODEJS_THIRD_PARTY_LICENSES.md
