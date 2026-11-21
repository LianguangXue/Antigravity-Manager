import { useEffect, useState } from 'react';
import { Network, RefreshCw, Check, X, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../../utils/request';
import { SshRemoteConfig } from '../../types/config';

interface SshRemoteSettingsProps {
    config?: SshRemoteConfig;
    onChange: (config: SshRemoteConfig) => void;
}

const DEFAULT_CONFIG: SshRemoteConfig = {
    enabled: false,
    hosts: [],
    restart_client: true,
};

type TestState = 'ok' | 'fail';

const SshRemoteSettings = ({ config = DEFAULT_CONFIG, onChange }: SshRemoteSettingsProps) => {
    const { i18n } = useTranslation();
    const isZh = (i18n.language || '').toLowerCase().startsWith('zh');
    const L = (en: string, zh: string) => (isZh ? zh : en);

    const [availableHosts, setAvailableHosts] = useState<string[]>([]);
    const [testingHost, setTestingHost] = useState<string | null>(null);
    const [testResults, setTestResults] = useState<Record<string, TestState>>({});

    useEffect(() => {
        invoke<string[]>('list_ssh_remote_hosts')
            .then((hosts) => setAvailableHosts(Array.isArray(hosts) ? hosts : []))
            .catch(() => setAvailableHosts([]));
    }, []);

    const allHosts = Array.from(new Set([...availableHosts, ...(config.hosts || [])])).sort();

    const toggleHost = (host: string) => {
        const selected = config.hosts || [];
        const hosts = selected.includes(host)
            ? selected.filter((item) => item !== host)
            : [...selected, host];
        onChange({ ...config, hosts });
    };

    const runTest = async (host: string) => {
        setTestingHost(host);
        try {
            await invoke<string>('test_ssh_remote_host', { host });
            setTestResults((prev) => ({ ...prev, [host]: 'ok' }));
        } catch {
            setTestResults((prev) => ({ ...prev, [host]: 'fail' }));
        } finally {
            setTestingHost(null);
        }
    };

    return (
        <div className="space-y-4">
            <div className="flex items-start justify-between gap-4">
                <div>
                    <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
                        <Network className="w-4 h-4 text-violet-500" />
                        {L('Remote SSH Sync', '远程 SSH 联动')}
                    </h3>
                    <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 leading-5">
                        {L(
                            'Push the selected account to Antigravity language servers running on SSH hosts, so Remote-SSH windows keep working after an account switch.',
                            '切换账号时把凭据同步到 SSH 主机上运行的 Antigravity 语言服务器，让 Remote-SSH 窗口跟着换号，不用重新登录。'
                        )}
                    </p>
                </div>
                <button
                    type="button"
                    onClick={() => onChange({ ...config, enabled: !config.enabled })}
                    className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors ${config.enabled ? 'bg-violet-500' : 'bg-gray-300 dark:bg-gray-600'}`}
                    aria-label={L('Enable remote SSH sync', '启用远程 SSH 联动')}
                >
                    <span
                        className={`inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${config.enabled ? 'translate-x-5' : 'translate-x-0.5'}`}
                    />
                </button>
            </div>

            <div className={config.enabled ? 'space-y-4' : 'space-y-4 opacity-50 pointer-events-none'}>
                <div>
                    <div className="flex items-center justify-between mb-2">
                        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            {L('SSH hosts', 'SSH 主机')}
                        </span>
                        <button
                            type="button"
                            onClick={() => {
                                invoke<string[]>('list_ssh_remote_hosts')
                                    .then((hosts) => setAvailableHosts(Array.isArray(hosts) ? hosts : []))
                                    .catch(() => setAvailableHosts([]));
                            }}
                            className="text-xs text-violet-600 dark:text-violet-400 hover:underline flex items-center gap-1"
                        >
                            <RefreshCw className="w-3 h-3" />
                            {L('Refresh', '刷新')}
                        </button>
                    </div>

                    {allHosts.length === 0 ? (
                        <p className="text-xs text-gray-500 dark:text-gray-400">
                            {L(
                                'No hosts found in ~/.ssh/config.',
                                '没有在 ~/.ssh/config 里找到主机。'
                            )}
                        </p>
                    ) : (
                        <div className="space-y-1.5">
                            {allHosts.map((host) => {
                                const checked = (config.hosts || []).includes(host);
                                const state = testResults[host];
                                return (
                                    <div
                                        key={host}
                                        className="flex items-center justify-between rounded-lg border border-gray-100 dark:border-base-200 px-3 py-2"
                                    >
                                        <label className="flex items-center gap-2 cursor-pointer min-w-0">
                                            <input
                                                type="checkbox"
                                                checked={checked}
                                                onChange={() => toggleHost(host)}
                                                className="rounded border-gray-300 text-violet-500 focus:ring-violet-500"
                                            />
                                            <span className="text-sm text-gray-700 dark:text-gray-200 truncate font-mono">
                                                {host}
                                            </span>
                                            {state === 'ok' && (
                                                <span className="text-emerald-500 flex items-center gap-1 text-xs">
                                                    <Check className="w-3 h-3" />
                                                    {L('reachable', '可达')}
                                                </span>
                                            )}
                                            {state === 'fail' && (
                                                <span className="text-rose-500 flex items-center gap-1 text-xs">
                                                    <X className="w-3 h-3" />
                                                    {L('failed', '失败')}
                                                </span>
                                            )}
                                        </label>
                                        <button
                                            type="button"
                                            onClick={() => runTest(host)}
                                            disabled={testingHost === host}
                                            className="text-xs text-gray-500 hover:text-violet-600 dark:text-gray-400 disabled:opacity-50 flex items-center gap-1"
                                        >
                                            {testingHost === host && (
                                                <Loader2 className="w-3 h-3 animate-spin" />
                                            )}
                                            {L('Test', '测试')}
                                        </button>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>

                <div className="flex items-start justify-between gap-4 rounded-lg border border-gray-100 dark:border-base-200 px-3 py-2.5">
                    <div>
                        <div className="text-sm font-medium text-gray-700 dark:text-gray-300">
                            {L('Restart client after switch', '切换后自动重启反重力')}
                        </div>
                        <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">
                            {L(
                                'Recommended: the client reconnects Remote-SSH windows automatically (takes ~10s). Turn off to only disconnect and reconnect manually.',
                                '推荐开启：客户端会自动重连 Remote-SSH 窗口（约 10 秒）。关闭后只断开连接，需要你在窗口里点一次 Reconnect。'
                            )}
                        </p>
                    </div>
                    <button
                        type="button"
                        onClick={() => onChange({ ...config, restart_client: !config.restart_client })}
                        className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors ${config.restart_client ? 'bg-violet-500' : 'bg-gray-300 dark:bg-gray-600'}`}
                        aria-label={L('Restart client after switch', '切换后自动重启反重力')}
                    >
                        <span
                            className={`inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${config.restart_client ? 'translate-x-5' : 'translate-x-0.5'}`}
                        />
                    </button>
                </div>
            </div>
        </div>
    );
};

export default SshRemoteSettings;
