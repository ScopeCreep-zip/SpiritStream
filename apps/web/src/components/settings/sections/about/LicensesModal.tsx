import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';

interface LicensesModalProps {
  open: boolean;
  onClose: () => void;
  appVersion: string;
}

/**
 * GPL-compliance third-party licenses modal. FFmpeg is the load-bearing
 * GPL dep (BtbN's GPL build on Windows / Linux for hardware encoders;
 * evermeet's GPL build on macOS). This modal documents the bundled
 * version and links to the matching source tarball attached to the
 * GitHub release.
 */
export function LicensesModal({ open, onClose, appVersion }: LicensesModalProps) {
  const { t } = useTranslation();
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('settings.thirdPartyLicensesTitle', { defaultValue: 'Third-party licenses' })}
      footer={
        <Button variant="ghost" onClick={onClose}>
          {t('common.close', { defaultValue: 'Close' })}
        </Button>
      }
    >
      <div className="flex flex-col gap-4 text-sm">
        <p className="text-text-secondary">
          {t('settings.thirdPartyLicensesIntro', {
            defaultValue:
              'SpiritStream bundles the following third-party software. License notices and corresponding source code are linked below.',
          })}
        </p>
        <div className="rounded-md border border-border-subtle p-3">
          <div className="flex items-baseline justify-between gap-2">
            <div className="font-medium">FFmpeg</div>
            <div className="text-xs text-text-tertiary">GPL v2 or later</div>
          </div>
          <div className="text-xs text-text-tertiary mt-1">
            {t('settings.thirdPartyLicensesFfmpegBundled', {
              defaultValue: 'Bundled with this build of SpiritStream.',
            })}
          </div>
          <ul className="mt-3 text-xs flex flex-col gap-1">
            <li>
              <a
                href="https://www.ffmpeg.org/legal.html"
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent-text underline"
              >
                {t('settings.thirdPartyLicensesFfmpegLicense', {
                  defaultValue: 'License (ffmpeg.org/legal.html)',
                })}
              </a>
            </li>
            <li>
              <a
                href={`https://github.com/ScopeCreep-zip/SpiritStream/releases/tag/v${appVersion || '0.0.0'}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent-text underline"
              >
                {t('settings.thirdPartyLicensesFfmpegSource', {
                  defaultValue: 'Matching source tarball (attached to this release)',
                })}
              </a>
            </li>
            <li>
              <a
                href="https://www.ffmpeg.org/download.html"
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent-text underline"
              >
                {t('settings.thirdPartyLicensesFfmpegUpstream', {
                  defaultValue: 'Upstream binary distributors',
                })}
              </a>
            </li>
          </ul>
        </div>
      </div>
    </Modal>
  );
}
