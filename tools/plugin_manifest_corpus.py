#!/usr/bin/env python3
"""Read-only, bounded inventory of immediate plugin manifest.json files under two roots."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import tempfile
import unittest

MAX_MANIFEST = 1024 * 1024


def safe_relative(value):
    if not isinstance(value, str) or not value or len(value) > 1024:
        return False
    if any(ord(c) < 32 or ord(c) == 127 for c in value):
        return False
    if '\\' in value or ':' in value or value.startswith('/'):
        return False
    return all(p not in ('', '.', '..') for p in value.split('/'))


def paths_ok(obj):
    """Validate known executable/resource path keys, including paths in actions/states."""
    if isinstance(obj, dict):
        for key, value in obj.items():
            if key in ('CodePath', 'CodePathWin', 'PropertyInspectorPath', 'Icon', 'Image'):
                if value is not None and not safe_relative(value):
                    return False
            if not paths_ok(value):
                return False
    elif isinstance(obj, list):
        return all(paths_ok(value) for value in obj)
    return True


def inspect(path):
    try:
        if path.is_symlink() or path.is_junction() or not path.is_file():
            return None, 'unsafe'
        with path.open('rb') as f:
            raw = f.read(MAX_MANIFEST + 1)
        if len(raw) > MAX_MANIFEST:
            return None, 'oversize'
        if raw.startswith(b'ELGATO\x01'):
            return None, 'encrypted'
        obj = json.loads(raw.decode('utf-8-sig'))
        if not isinstance(obj, dict) or not paths_ok(obj):
            return None, 'invalid'
        version = obj.get('Version')
        uuid = obj.get('UUID') or path.parent.name.removesuffix('.sdPlugin')
        if not isinstance(uuid, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,199}', uuid):
            return None, 'invalid'
        if not isinstance(version, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.+-]{0,99}', version):
            return None, 'invalid'
        actions = obj.get('Actions', [])
        if not isinstance(actions, list) or not all(isinstance(a, dict) for a in actions):
            return None, 'invalid'
        os_req = obj.get('OS', [])
        if os_req is None:
            os_req = []
        if not isinstance(os_req, list) or not all(isinstance(x, dict) for x in os_req):
            return None, 'invalid'
        apps = obj.get('ApplicationsToMonitor', [])
        if apps is None:
            apps = []
        if not isinstance(apps, list):
            return None, 'invalid'
        node = obj.get('Nodejs')
        if node is not None and not isinstance(node, dict):
            return None, 'invalid'
        code = obj.get('CodePathWin') or obj.get('CodePath') or ''
        if code and not safe_relative(code):
            return None, 'invalid'
        runtime = 'exe' if code.lower().endswith('.exe') else 'node' if code.lower().endswith(('.js', '.mjs', '.cjs')) else 'html' if code.lower().endswith(('.html', '.htm')) else 'none/other'
        return {
            'key': (uuid, version, hashlib.sha256(raw).hexdigest()),
            'runtime': runtime,
            'actions': len(actions),
            'os': bool(os_req),
            'node': bool(node and node.get('Version')),
            'inspector': bool(obj.get('PropertyInspectorPath')),
            'apps': len(apps),
        }, None
    except (OSError, UnicodeError, ValueError, TypeError, RecursionError):
        return None, 'invalid'


def inventory(roots):
    counts = Counter()
    unique = {}
    for root in roots:
        if not root.is_dir() or root.is_symlink() or root.is_junction():
            counts['missing/unsafe root'] += 1
            continue
        try:
            children = sorted(root.iterdir())
        except OSError:
            counts['missing/unsafe root'] += 1
            continue
        for child in children:
            if child.is_symlink() or child.is_junction() or not child.is_dir():
                continue
            manifest = child / 'manifest.json'
            if not manifest.exists() and not manifest.is_symlink():
                continue
            counts['found'] += 1
            record, error = inspect(manifest)
            if error:
                counts['rejected ' + error] += 1
            elif record['key'] in unique:
                counts['duplicates'] += 1
            else:
                unique[record['key']] = record
    counts['unique'] = len(unique)
    for record in unique.values():
        counts['runtime ' + record['runtime']] += 1
        counts['actions'] += record['actions']
        counts['OS prerequisites'] += record['os']
        counts['Node version prerequisites'] += record['node']
        counts['PropertyInspectorPath'] += record['inspector']
        counts['ApplicationsToMonitor entries'] += record['apps']
        counts['ApplicationsToMonitor manifests'] += bool(record['apps'])
    fingerprint = hashlib.sha256(('\n'.join(sorted(x[2] for x in unique)) + '\n').encode()).hexdigest()
    return counts, fingerprint


def markdown(roots):
    counts, fingerprint = inventory(roots)
    keys = ('found', 'unique', 'duplicates', 'rejected encrypted', 'rejected invalid',
            'rejected unsafe', 'rejected oversize', 'missing/unsafe root',
            'runtime exe', 'runtime node', 'runtime html', 'runtime none/other',
            'actions', 'OS prerequisites', 'Node version prerequisites',
            'PropertyInspectorPath', 'ApplicationsToMonitor manifests',
            'ApplicationsToMonitor entries')
    lines = ['# Corpus de manifestes plugins — inventaire statique', '',
             'Date UTC : ' + datetime.now(timezone.utc).strftime('%Y-%m-%d'),
             'Empreinte SHA256 du corpus (SHA256 des SHA256 des manifestes uniques triés, séparés par LF et terminés par LF) : `' + fingerprint + '`',
             '', '| Mesure | Nombre |', '| --- | ---: |']
    lines += [f'| {key} | {counts[key]} |' for key in keys]
    lines += ['', 'Deux racines locales fournies en arguments ; seuls les manifest.json des sous-dossiers immédiats sont lus (1 Mio maximum chacun). Aucun fichier de ressource, de code ou de réglages de plugin lu ; aucun plugin exécuté, copié ou modifié. Les chemins symboliques/jonctions sont ignorés. Les manifestes chiffrés, illisibles, illégaux ou aux chemins connus risqués sont rejetés ; déduplication par UUID, version et SHA256 du manifeste brut. Les champs inconnus ne sont pas interprétés.', '',
              'Les nombres décrivent les déclarations des manifestes, pas la présence des exécutables, des logiciels requis ni le fonctionnement. Aucune conformité runtime ou compatibilité DeckForge/SDK n’est prouvée. Aucun appui, événement willAppear, appel réseau ou lancement d’application.', '']
    return '\n'.join(lines)


class CorpusTests(unittest.TestCase):
    def test_reject_and_deduplicate(self):
        with tempfile.TemporaryDirectory() as temp:
            roots = [Path(temp) / 'one', Path(temp) / 'two']
            for root in roots:
                (root / 'plugin.sdPlugin').mkdir(parents=True)
                (root / 'plugin.sdPlugin' / 'manifest.json').write_text(json.dumps({
                    'Version': '1.0', 'CodePathWin': 'bin/main.exe', 'CodePath': 'other.js',
                    'Actions': [{}, {}], 'OS': [{'Platform': 'windows'}],
                    'Nodejs': {'Version': '20'}, 'PropertyInspectorPath': 'property inspector/main.html',
                    'ApplicationsToMonitor': ['sample.exe']}), encoding='utf-8-sig')
            (roots[0] / 'bad.sdPlugin').mkdir()
            (roots[0] / 'bad.sdPlugin' / 'manifest.json').write_bytes(b'ELGATO\x01secret')
            (roots[0] / 'traversal.sdPlugin').mkdir()
            (roots[0] / 'traversal.sdPlugin' / 'manifest.json').write_text(json.dumps({'UUID':'safe','Version':'1','CodePath':'../escape.exe'}))
            counts, digest = inventory(roots)
            self.assertEqual((counts['found'], counts['unique'], counts['duplicates'], counts['rejected encrypted'], counts['rejected invalid']), (4, 1, 1, 1, 1))
            self.assertEqual((counts['runtime exe'], counts['actions'], counts['OS prerequisites'], counts['Node version prerequisites'], counts['PropertyInspectorPath'], counts['ApplicationsToMonitor entries']), (1, 2, 1, 1, 1, 1))
            self.assertEqual(len(digest), 64)
            report = markdown(roots)
            self.assertNotIn(temp, report)
            self.assertNotIn('sample.exe', report)

    def test_path_and_invalid_json(self):
        for value in ('../x', '/tmp/x', 'C:/x', 'a\\b', 'a//b', 'a/./b', 'a/../b', 'x\x00y'):
            self.assertFalse(safe_relative(value))
        self.assertTrue(safe_relative('property inspector/index.html'))
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'manifest.json'
            p.write_bytes(b'{bad')
            self.assertEqual(inspect(p)[1], 'invalid')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test', action='store_true')
    parser.add_argument('roots', nargs='*', type=Path)
    args = parser.parse_args()
    if args.test:
        unittest.main(argv=['corpus tests'])
    else:
        if len(args.roots) != 2:
            parser.error('exactly two roots required')
        print(markdown(args.roots))
