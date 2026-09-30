import 'dart:convert';
import 'dart:io';

import 'package:codex_timezone/services/backend.dart';

Future<void> main() async {
  if (!Platform.resolvedExecutable.contains('native-acceptance-')) {
    throw StateError('Run from an isolated native-acceptance directory.');
  }
  const backend = NativeBackend();
  final settings = {
    'mode': 'zone',
    'zoneId': 'America/Los_Angeles',
    'offset': -8,
    'executable': '',
    'dreamSkinCompatible': false,
  };
  final current = await backend.call('check_launch', {'settings': settings});
  if (current['healthy'] != true) {
    throw StateError('Current package is not healthy: $current');
  }
  final package = (current['target'] as Map)['package'] as Map;
  final stale = {
    ...settings,
    'executable': r'C:\Program Files\WindowsApps\OpenAI.Codex_0.0.0.1_x64__2p2nqsd0c76g0\app\ChatGPT.exe',
  };
  final updated = await backend.call('check_launch', {'settings': stale});
  if (updated['healthy'] != true ||
      jsonEncode(updated['target']) != jsonEncode(current['target'])) {
    throw StateError(
      'Stale version did not resolve to the current registered package.',
    );
  }
  final noOp = await backend.call('repair_launch', {
    'settings': stale,
    'packageFullName': package['fullName'],
  });
  if (noOp['healthy'] != true || noOp['repaired'] != false) {
    throw StateError('Healthy package must skip ACL repair and UAC.');
  }
  var staleRejected = false;
  try {
    await backend.call('repair_launch', {
      'settings': stale,
      'packageFullName': 'OpenAI.Codex_0.0.0.1_x64__2p2nqsd0c76g0',
    });
  } catch (error) {
    staleRejected = error.toString().contains('已更新');
  }
  if (!staleRejected) {
    throw StateError('Repair of an outdated version was not rejected.');
  }
  stdout.writeln(
    jsonEncode({
      'status': 'passed',
      'target': current['target'],
      'stalePathUpdated': true,
      'healthyRepairSkipped': true,
      'staleRepairRejected': true,
      'applicationResumed': false,
    }),
  );
}
