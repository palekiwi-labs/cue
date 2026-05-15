1. Update MemType enum in src/cli.rs to include Bin variant.
2. Update destination directory logic in src/commands/add.rs to handle MemType::Bin by mapping it to the 'bin' directory.
3. Add an integration test in tests/add.rs to verify that --type bin works as expected.