// prepublishOnly guard: a bare `npm publish` from this checkout packs the
// working tree after `prepare` rebuilds node/dist with this host's addon only,
// which is not the audited release tarball. npm runs no lifecycle scripts when
// publishing a tarball, so scripts/release/publish-registries.sh is unaffected.
console.error(
  'refusing a bare `npm publish` from the checkout: it would upload the working tree, not the release.\n' +
    'Publish the built tarball with scripts/release/publish-registries.sh <version>.',
);
process.exit(1);
