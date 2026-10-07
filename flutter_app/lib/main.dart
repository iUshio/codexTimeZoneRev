import 'package:flutter/widgets.dart';
import 'package:timezone/data/latest_all.dart' as timezone;

import 'app/application.dart';
import 'services/window_material.dart';

Future<void> main(List<String> arguments) async {
  WidgetsFlutterBinding.ensureInitialized();
  timezone.initializeTimeZones();
  final acrylic = await WindowMaterial.initialize(
    enabled: !arguments.contains('--solid-background'),
  );
  runApp(
    LauncherApplication(
      acrylicEnabled: acrylic,
      preview:
          arguments.contains('--preview') ||
          const bool.fromEnvironment('UI_PREVIEW'),
    ),
  );
}
