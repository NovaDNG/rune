import 'package:fluent_ui/fluent_ui.dart';

import '../../providers/responsive_providers.dart';

import 'settings_block.dart';
import 'settings_block_title.dart';

class SettingsBoxBase extends StatelessWidget {
  const SettingsBoxBase({
    super.key,
    required this.title,
    required this.subtitle,
    required this.buildExpanderContent,
    required this.buildDefaultContent,
    this.icon,
    this.iconColor,
  });

  final String title;
  final String subtitle;
  final IconData? icon;
  final Color? iconColor;
  final Widget Function(BuildContext) buildExpanderContent;
  final Widget Function(BuildContext) buildDefaultContent;

  @override
  Widget build(BuildContext context) {
    return SmallerOrEqualTo(
      deviceType: DeviceType.zune,
      builder: (context, isZune) {
        if (isZune) {
          return Padding(
            padding: const EdgeInsets.all(4),
            child: Expander(
              header: Padding(
                padding: const EdgeInsets.symmetric(vertical: 11),
                child: SettingsBlockTitle(
                  title: title,
                  subtitle: subtitle,
                ),
              ),
              content: buildExpanderContent(context),
            ),
          );
        }
        return SettingsBlock(
          icon: icon,
          iconColor: iconColor,
          title: title,
          subtitle: subtitle,
          child: buildDefaultContent(context),
        );
      },
    );
  }
}
