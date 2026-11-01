"""Shared content identity for the native executable's compilation inputs."""
from hashlib import sha256


def native_inputs(root):
    # The patched crate is real compilation input. A safe vendor-tree superset
    # also covers build scripts, manifests, WGSL includes and upstream notices
    # referenced by source, while excluding generated dependency build caches.
    ignored_parts = {'target', '.git', '__pycache__', 'node_modules'}
    return sorted(
        [root / 'native/Cargo.toml', root / 'native/Cargo.lock']
        + [p for p in (root / 'assets/icons').rglob('*') if p.is_file() and p.suffix == '.json']
        + [p for folder in ('src', 'shaders', 'assets', 'vendor', 'qualification')
           for p in (root / 'native' / folder).rglob('*')
           if p.is_file() and not ignored_parts.intersection(p.relative_to(root / 'native').parts)],
        # Component tuples keep Linux's existing case-sensitive ordering on
        # every host; Windows Path ordering would otherwise fold case.
        key=lambda p: tuple(p.relative_to(root).parts),
    )


def source_digest(root):
    digest = sha256()
    for source in native_inputs(root):
        digest.update(source.relative_to(root).as_posix().encode())
        digest.update(b'\0')
        digest.update(source.read_bytes())
    return digest.hexdigest()
