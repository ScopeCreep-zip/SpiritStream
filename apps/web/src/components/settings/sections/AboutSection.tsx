import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Github, BookOpen } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Logo } from '@/components/layout/Logo';
import { api } from '@/lib/client';
import { UpdaterButton, UpdaterStateView, useUpdaterController } from './about/UpdaterCard';
import { LicensesModal } from './about/LicensesModal';

/**
 * App version + GitHub/docs links + the self-updater state machine +
 * GPL-compliance third-party licenses modal entrypoint.
 *
 * The 7-state updater lives in {@link UpdaterCard}; its state is owned
 * here via {@link useUpdaterController} so the inline check-for-updates
 * button (rendered next to Github/Docs) and the below-the-row status
 * surface share one state instance. The GPL modal lives in
 * {@link LicensesModal}.
 */
export function AboutSection() {
  const { t } = useTranslation();
  const [appVersion, setAppVersion] = useState<string>('');
  const [updaterSupported, setUpdaterSupported] = useState<boolean | null>(null);
  const [licensesModalOpen, setLicensesModalOpen] = useState(false);
  const updater = useUpdaterController();

  // Probe app version + updater support once on mount; both are stable
  // for the lifetime of the process.
  useEffect(() => {
    let cancelled = false;
    api.system
      .appVersion()
      .then(({ version }) => {
        if (!cancelled) setAppVersion(version);
      })
      .catch(() => {});
    import('@/utils/selfUpdate')
      .then(({ isUpdaterSupported }) => isUpdaterSupported())
      .then((supported) => {
        if (!cancelled) setUpdaterSupported(supported);
      })
      .catch(() => {
        if (!cancelled) setUpdaterSupported(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <>
      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('settings.about')}</CardTitle>
            <CardDescription>{t('settings.aboutDescription')}</CardDescription>
          </div>
        </CardHeader>
        <CardBody>
          <div className="text-center py-4">
            <div className="flex justify-center mb-4">
              <Logo size="lg" />
            </div>
            <div className="text-sm text-text-secondary mb-1">
              {t('settings.version')} {appVersion || '…'}
            </div>
            <div className="text-xs text-text-tertiary mb-6">{t('settings.tagline')}</div>
            <div className="flex justify-center gap-3">
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  window.open(
                    'https://github.com/ScopeCreep-zip/SpiritStream',
                    '_blank',
                    'noopener,noreferrer'
                  )
                }
              >
                <Github className="w-4 h-4" />
                {t('settings.github')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  window.open(
                    'https://deepwiki.com/ScopeCreep-zip/SpiritStream',
                    '_blank',
                    'noopener,noreferrer'
                  )
                }
              >
                <BookOpen className="w-4 h-4" />
                {t('settings.docs')}
              </Button>
              <UpdaterButton updaterSupported={updaterSupported} controller={updater} />
            </div>
            <div className="mt-3">
              <button
                type="button"
                className="text-xs text-text-tertiary underline hover:text-text-secondary"
                onClick={() => setLicensesModalOpen(true)}
              >
                {t('settings.thirdPartyLicenses', { defaultValue: 'Third-party licenses' })}
              </button>
            </div>
            <UpdaterStateView updaterSupported={updaterSupported} controller={updater} />
          </div>
        </CardBody>
      </Card>

      <LicensesModal
        open={licensesModalOpen}
        onClose={() => setLicensesModalOpen(false)}
        appVersion={appVersion}
      />
    </>
  );
}
