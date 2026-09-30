import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:codex_timezone/services/backend.dart';

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

Future<void> fails(Future<Object?> operation, String expected) async {
  try {
    await operation;
  } on StateError catch (error) {
    check(
      error.toString().contains(expected),
      'Unexpected native error: $error',
    );
    return;
  }
  throw StateError('Expected failure: $expected');
}

Future<void> main() async {
  if (!Platform.isWindows) throw UnsupportedError('Windows acceptance runner');
  final root = p.dirname(Platform.resolvedExecutable);
  // Never run configuration tests from the Dart SDK or a real app directory.
  check(
    p.basename(root).startsWith('native-acceptance-') &&
        File(p.join(root, '.acceptance-fixture')).existsSync(),
    'Isolated fixture directory required',
  );
  final data = Directory(p.join(root, 'data'))..createSync();
  final settingsFile = File(p.join(data.path, 'settings.json'));
  settingsFile.writeAsStringSync('\ufeff{"Mode":"offset","Offset":3}');
  const backend = NativeBackend();
  final initial = await backend.call('bootstrap');
  check(
    (initial['settings'] as Map)['offset'] == 3,
    'Legacy settings not loaded',
  );
  check(
    p.equals(initial['settingsPath'] as String, settingsFile.path),
    'Configuration escaped fixture directory',
  );
  check((initial['zones'] as List).isNotEmpty, 'Missing bundled zones');
  final executable = p.join(root, '模拟 客户端', 'Codex.exe');
  final validated = await backend.call('validate', {'path': executable});
  check(
    (validated['path'] as String).endsWith('Codex.exe'),
    'Fixture path was not validated',
  );
  var settings = <String, dynamic>{
    'mode': 'zone',
    'zoneId': 'Asia/Shanghai',
    'offset': 8,
    'executable': executable,
    'dreamSkinCompatible': false,
  };
  await backend.call('save', {'settings': settings});
  check(
    (jsonDecode(settingsFile.readAsStringSync()) as Map)['mode'] == 'zone',
    'Save did not persist',
  );
  final snapshot = settingsFile.readAsStringSync();
  await fails(
    backend.call('save', {
      'settings': {...settings, 'mode': 'offset', 'offset': 99},
    }),
    '偏移',
  );
  check(
    settingsFile.readAsStringSync() == snapshot,
    'Invalid save replaced valid configuration',
  );

  final report = File(p.join(p.dirname(executable), 'observed.txt'));
  final launch = await backend.call('launch', {'settings': settings});
  check(launch['launched'] == true, 'Fixture did not launch');
  check(
    launch['windowReadyVerified'] == false,
    'Process survival must not claim that the client window is ready',
  );
  check(
    launch['identityVerified'] == false &&
        (launch['target'] as Map)['package'] == null,
    'Unpackaged fixture was incorrectly reported as a verified MSIX app',
  );
  final launchLog = File(launch['logPath'] as String);
  check(
    p.equals(launchLog.path, p.join(data.path, 'launch.log')),
    'Launch diagnostics escaped the isolated data directory',
  );
  final logText = launchLog.readAsStringSync();
  final stages = logText
      .split('\n')
      .where((line) => line.trim().isNotEmpty)
      .map((line) => (jsonDecode(line) as Map)['stage'])
      .toList();
  check(
    stages.contains('created_suspended') &&
        stages.indexOf('identity_checked') >
            stages.indexOf('created_suspended') &&
        stages.indexOf('resumed') > stages.indexOf('identity_checked') &&
        stages.last == 'process_started',
    'Launch stage order does not prove identity was checked before resume',
  );
  check(
    !logText.contains('fixture-parent'),
    'Launch log dumped parent environment values',
  );
  final observed = report.readAsStringSync();
  check(observed.contains('TZ=Asia/Shanghai'), 'Child process TZ mismatch');
  check(
    observed.contains('ELECTRON_RUN_AS_NODE=<unset>'),
    'Electron variable leaked to child',
  );
  check(observed.contains('模拟 客户端'), 'Child working directory mismatch');
  await fails(backend.call('launch', {'settings': settings}), '正在运行');
  check(Platform.environment['TZ'] == 'Etc/UTC', 'Parent TZ was modified');
  check(
    Platform.environment['ELECTRON_RUN_AS_NODE'] == 'fixture-parent',
    'Parent environment was modified',
  );
  await Future<void>.delayed(const Duration(milliseconds: 3300));

  settings = {...settings, 'mode': 'offset', 'offset': -12};
  await backend.call('launch', {'settings': settings});
  check(
    report.readAsStringSync().contains('TZ=Etc/GMT+12'),
    'Fixed-offset POSIX sign mismatch',
  );
  await Future<void>.delayed(const Duration(milliseconds: 3300));
  final persisted = settingsFile.readAsStringSync();
  await fails(
    backend.call('launch', {
      'settings': {...settings, 'executable': p.join(root, 'missing.exe')},
    }),
    '不存在',
  );
  check(
    settingsFile.readAsStringSync() == persisted,
    'Failed validation unexpectedly saved settings',
  );

  // A package-looking path must never fall back to an ordinary executable when
  // no matching package is registered. This fixture is never launched.
  final unregistered = Directory(p.join(root, 'unregistered-package'))
    ..createSync();
  final unregisteredExe = p.join(unregistered.path, 'Codex.exe');
  File(executable).copySync(unregisteredExe);
  File(p.join(unregistered.path, 'icudtl.dat')).writeAsStringSync('');
  File(p.join(unregistered.path, 'AppxManifest.xml')).writeAsStringSync(
    '<Package><Applications><Application Id="App" '
    'Executable="Codex.exe" /></Applications></Package>',
  );
  await fails(
    backend.call('launch', {
      'settings': {...settings, 'executable': unregisteredExe},
    }),
    '注册',
  );
  check(
    !File(p.join(unregistered.path, 'observed.txt')).existsSync(),
    'Unregistered package-looking fixture executed',
  );
  check(
    settingsFile.readAsStringSync() == persisted,
    'Rejected package-looking target changed saved settings',
  );

  settingsFile.writeAsStringSync('{invalid json');
  await fails(backend.call('bootstrap'), '设置文件无效');
  check(
    settingsFile.readAsStringSync() == '{invalid json',
    'Bootstrap overwrote damaged settings',
  );
  settingsFile.writeAsStringSync(persisted);
  await backend.call('bootstrap');
  final proxy = await backend.call('proxy_for_url', {
    'url': 'https://example.com/',
  });
  check(proxy['proxy'] is String, 'System proxy lookup failed');
  stdout.writeln(
    'Windows native acceptance passed: legacy bootstrap, isolated save, path validation, suspended fixture launch, identity gate ordering, child TZ, environment cleanup, truthful process status, scoped diagnostics, duplicate launch rejection, unregistered package rejection, invalid save/launch protection, bootstrap retry, system proxy lookup.',
  );
}
