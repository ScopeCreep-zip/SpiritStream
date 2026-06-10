import React from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Copy } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { PasswordInput } from '@spiritstream/ui';

interface ObsConnectionFormProps {
  isConnected: boolean;

  host: string;
  setHost: (value: string) => void;
  onHostBlur: () => void;

  port: string;
  setPort: (value: string) => void;
  onPortBlur: () => void;

  useAuth: boolean;
  onUseAuthChange: (checked: boolean) => void | Promise<void>;

  password: string;
  setPassword: (value: string) => void;
  onPasswordBlur: () => void;

  showPassword: boolean;
  setShowPassword: (visible: boolean) => void;
  onCopyPassword: () => void;

  autoConnect: boolean;
  onAutoConnectChange: (checked: boolean) => void | Promise<void>;
}

export function ObsConnectionForm({
  isConnected,
  host,
  setHost,
  onHostBlur,
  port,
  setPort,
  onPortBlur,
  useAuth,
  onUseAuthChange,
  password,
  setPassword,
  onPasswordBlur,
  showPassword,
  setShowPassword,
  onCopyPassword,
  autoConnect,
  onAutoConnectChange,
}: ObsConnectionFormProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('obs.connectionSettings')}</CardTitle>
      </CardHeader>
      <CardBody className="space-y-4">
        <div className="grid grid-cols-3 gap-3">
          <div className="col-span-2">
            <Input
              label={t('obs.host')}
              value={host}
              onChange={(e) => setHost(e.target.value)}
              onBlur={onHostBlur}
              placeholder={t('obs.hostPlaceholder')}
              disabled={isConnected}
            />
          </div>
          <div>
            <Input
              label={t('obs.port')}
              type="number"
              value={port}
              onChange={(e) => setPort(e.target.value)}
              onBlur={onPortBlur}
              disabled={isConnected}
            />
          </div>
        </div>

        <Toggle
          checked={useAuth}
          onChange={onUseAuthChange}
          label={t('obs.useAuthentication')}
          disabled={isConnected}
        />

        {useAuth && (
          <div className="flex items-end gap-1">
            <div className="flex-1">
              <PasswordInput
                label={t('obs.password')}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onBlur={onPasswordBlur}
                placeholder={t('obs.passwordPlaceholder')}
                disabled={isConnected}
                visible={showPassword}
                onVisibilityChange={setShowPassword}
                showLabel={t('obs.showPassword')}
                hideLabel={t('obs.hidePassword')}
                renderToggleIcon={(visible) =>
                  visible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />
                }
              />
            </div>
            <Button
              variant="ghost"
              size="icon"
              onClick={onCopyPassword}
              aria-label={t('common.copy')}
              disabled={isConnected || !password}
            >
              <Copy className="w-4 h-4" />
            </Button>
          </div>
        )}

        <Toggle checked={autoConnect} onChange={onAutoConnectChange} label={t('obs.autoConnect')} />
      </CardBody>
    </Card>
  );
}
