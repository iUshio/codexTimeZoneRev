import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:codex_timezone/services/backend.dart';
import 'package:codex_timezone/state/launcher_controller.dart';

class FakeBackend implements LauncherBackend {
  bool failBootstrap = false, failSave = false;
  Completer<Map<String, dynamic>>? pending;
  final List<String> calls = [];
  final List<Map<String, dynamic>> payloads = [];
  final Map<String, Map<String, dynamic>> responses = {};
  String? failCommand;
  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) async {
    calls.add(command);
    payloads.add(payload);
    if (command == 'bootstrap') {
      if (failBootstrap) throw StateError('损坏配置');
      return const PreviewBackend().call(command);
    }
    if (command == 'validate') return {'path': payload['path']};
    if (failSave) throw StateError('保存失败');
    if (command == failCommand) throw StateError('repair failed');
    if (pending != null) return pending!.future;
    if (responses.containsKey(command)) return responses[command]!;
    return {'message': '完成'};
  }
}

void main() {
  const needsRepair = <String, dynamic>{
    'repairRequired': true,
    'launched': false,
    'target': {
      'executable': r'C:\WindowsApps\current\app\ChatGPT.exe',
      'package': {'fullName': 'OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0'},
    },
    'message': '需要修复',
    'logPath': r'C:\launcher\data\launch.log',
  };

  Future<LauncherController> prepareRepair(FakeBackend backend) async {
    backend.responses['launch'] = needsRepair;
    final controller = LauncherController(backend);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.update(controller.settings.copyWith(zoneId: 'America/New_York'));
    expect(await controller.action('launch'), isFalse);
    expect(controller.repairPackage, isNotNull);
    expect(controller.dirty, isTrue);
    expect(backend.calls.where((c) => c == 'repair_launch'), isEmpty);
    return controller;
  }

  test(
    'repair is explicit, version bound and retries once with the same timezone',
    () async {
      final backend = FakeBackend();
      final controller = await prepareRepair(backend);
      backend.responses['repair_launch'] = {'healthy': true};
      final saved = controller.settings.copyWith(
        executable: r'C:\new\ChatGPT.exe',
      );
      backend.responses['launch'] = {
        'launched': true,
        'settings': saved.toJson(),
      };
      expect(await controller.action('repair_launch'), isTrue);
      expect(backend.calls, ['bootstrap', 'launch', 'repair_launch', 'launch']);
      expect(
        backend.payloads[2]['packageFullName'],
        'OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0',
      );
      expect(
        (backend.payloads[3]['settings'] as Map)['zoneId'],
        'America/New_York',
      );
      expect(controller.settings.executable, saved.executable);
      expect(controller.repairPackage, isNull);
      expect(controller.dirty, isFalse);
    },
  );

  test('UAC cancellation retains draft and never retries launch', () async {
    final backend = FakeBackend();
    final controller = await prepareRepair(backend);
    backend.responses['repair_launch'] = {'cancelled': true, 'message': '已取消'};
    expect(await controller.action('repair_launch'), isFalse);
    expect(backend.calls, ['bootstrap', 'launch', 'repair_launch']);
    expect(controller.message, '已取消');
    expect(controller.dirty, isTrue);
    expect(controller.busy, isFalse);
  });

  test(
    'repair failure requires a fresh check and never retries launch',
    () async {
      final backend = FakeBackend();
      final controller = await prepareRepair(backend);
      backend.failCommand = 'repair_launch';
      expect(await controller.action('repair_launch'), isFalse);
      expect(controller.repairPackage, isNull);
      expect(controller.dirty, isTrue);
      expect(await controller.action('repair_launch'), isFalse);
      expect(backend.calls, ['bootstrap', 'launch', 'repair_launch']);
    },
  );

  test('identity regression during retry cannot cause a repair loop', () async {
    final backend = FakeBackend();
    final controller = await prepareRepair(backend);
    backend.responses['repair_launch'] = {'healthy': true};
    expect(await controller.action('repair_launch'), isFalse);
    expect(backend.calls, ['bootstrap', 'launch', 'repair_launch', 'launch']);
    expect(controller.repairPackage, isNotNull);
    expect(controller.dirty, isTrue);
  });

  test(
    'editing the target or timezone invalidates the pending repair',
    () async {
      final backend = FakeBackend();
      final controller = await prepareRepair(backend);
      controller.update(
        controller.settings.copyWith(executable: r'C:\other\Codex.exe'),
      );
      expect(controller.repairPackage, isNull);
      expect(await controller.action('repair_launch'), isFalse);
      expect(backend.calls, ['bootstrap', 'launch']);
    },
  );

  test('pending repair blocks duplicate requests', () async {
    final backend = FakeBackend();
    final controller = await prepareRepair(backend);
    backend.pending = Completer();
    final repair = controller.action('repair_launch');
    expect(await controller.action('repair_launch'), isFalse);
    expect(await controller.action('launch'), isFalse);
    backend.pending!.complete({'cancelled': true});
    expect(await repair, isFalse);
    expect(backend.calls, ['bootstrap', 'launch', 'repair_launch']);
  });

  test(
    'closing the launcher while repairing prevents automatic launch',
    () async {
      final backend = FakeBackend()..responses['launch'] = needsRepair;
      final controller = LauncherController(backend);
      await controller.initialize();
      await controller.action('launch');
      backend.pending = Completer();
      final operation = controller.action('repair_launch');
      controller.dispose();
      backend.pending!.complete({'healthy': true});
      expect(await operation, isFalse);
      expect(backend.calls, ['bootstrap', 'launch', 'repair_launch']);
    },
  );
  test(
    'file chooser cancellation releases busy state without modifying draft',
    () async {
      final backend = FakeBackend();
      final controller = LauncherController(backend);
      addTearDown(controller.dispose);
      await controller.initialize();
      final selected = Completer<String?>();
      final browse = controller.browse(() => selected.future);
      expect(controller.busy, isTrue);
      expect(await controller.action('save'), isFalse);
      var secondOpened = false;
      await controller.browse(() async {
        secondOpened = true;
        return null;
      });
      expect(secondOpened, isFalse);
      selected.complete(null);
      await browse;
      expect(controller.busy, isFalse);
      expect(controller.dirty, isFalse);
      expect(backend.calls, ['bootstrap']);
    },
  );

  test(
    'file chooser updates draft only after native path validation',
    () async {
      final backend = FakeBackend();
      final controller = LauncherController(backend);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.browse(() async => r'C:\测试 客户端\Codex.exe');
      expect(controller.settings.executable, r'C:\测试 客户端\Codex.exe');
      expect(controller.dirty, isTrue);
      expect(backend.calls, ['bootstrap', 'validate']);
    },
  );

  test(
    'closing the app during file selection does not validate or notify later',
    () async {
      final backend = FakeBackend();
      final controller = LauncherController(backend);
      await controller.initialize();
      final selected = Completer<String?>();
      final browse = controller.browse(() => selected.future);
      controller.dispose();
      selected.complete(r'C:\Codex.exe');
      await browse;
      expect(backend.calls, ['bootstrap']);
    },
  );
  test('failed bootstrap cannot save placeholders; retry recovers', () async {
    final backend = FakeBackend()..failBootstrap = true;
    final controller = LauncherController(backend);
    addTearDown(controller.dispose);
    await controller.initialize();
    expect(controller.bootstrapFailed, isTrue);
    expect(await controller.action('save'), isFalse);
    expect(backend.calls, ['bootstrap']);
    backend.failBootstrap = false;
    await controller.initialize();
    expect(controller.canSave, isTrue);
    expect(controller.dirty, isFalse);
  });

  test(
    'failed save retains draft and skin actions never mark it saved',
    () async {
      final backend = FakeBackend();
      final controller = LauncherController(backend);
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.update(
        controller.settings.copyWith(zoneId: 'America/New_York'),
      );
      expect(controller.dirty, isTrue);
      backend.failSave = true;
      expect(await controller.action('save'), isFalse);
      expect(controller.dirty, isTrue);
      backend.failSave = false;
      await controller.action('launch_dream_skin');
      expect(controller.dirty, isTrue);
      await controller.action('reapply_dream_skin');
      expect(controller.dirty, isTrue);
      expect(backend.calls.last, 'reapply_dream_skin');
      await controller.action('save');
      expect(controller.dirty, isFalse);
    },
  );

  test('in-flight launch blocks duplicate requests and draft edits', () async {
    final backend = FakeBackend();
    final controller = LauncherController(backend);
    addTearDown(controller.dispose);
    await controller.initialize();
    backend.pending = Completer();
    final original = controller.settings;
    final launch = controller.action('launch');
    expect(controller.busy, isTrue);
    controller.update(original.copyWith(offset: -5));
    expect(controller.settings, original);
    expect(await controller.action('launch'), isFalse);
    backend.pending!.complete({'message': '完成'});
    expect(await launch, isTrue);
    expect(backend.calls.where((call) => call == 'launch'), hasLength(1));
  });

  test('unsupported network timezone is rejected before persistence', () async {
    final backend = FakeBackend();
    final controller = LauncherController(backend);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.update(controller.settings.copyWith(zoneId: 'Unsupported/Zone'));
    expect(controller.canSave, isFalse);
    expect(await controller.action('save'), isFalse);
    expect(backend.calls, ['bootstrap']);
  });
}
