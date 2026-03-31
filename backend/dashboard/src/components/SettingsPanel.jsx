import React, { useState, useEffect } from 'react';
import { Settings, ShieldAlert, ShieldCheck, Play, Save, Zap, Sliders, Activity } from 'lucide-react';

const SettingsPanel = ({ config, secrets, sendCommand }) => {
    const [activeTab, setActiveTab] = useState('control');
    const [localConfig, setLocalConfig] = useState({
        margin_threshold_low: "0.4",
        margin_threshold_high: "0.7",
        concentration_threshold: "0.7",
        target_margin_ratio: "0.2",
        automated_rebalance_enabled: false
    });
    const [localSecrets, setLocalSecrets] = useState({
        binance_key: "", binance_secret: "",
        bybit_key: "", bybit_secret: "",
        bitget_key: "", bitget_secret: "", bitget_passphrase: "",
        gate_key: "", gate_secret: "",
        bitmart_key: "", bitmart_secret: "", bitmart_memo: "",
        kraken_key: "", kraken_secret: "",
        mexc_key: "", mexc_secret: "",
        okx_key: "", okx_secret: "", okx_passphrase: "",
        telegram_token: "", telegram_chat_id: ""
    });

    useEffect(() => { if (config) setLocalConfig(config); }, [config]);
    useEffect(() => { if (secrets) setLocalSecrets(secrets); }, [secrets]);

    const handleChange = (field, value) => setLocalConfig(prev => ({ ...prev, [field]: value }));
    const handleSecretChange = (field, value) => setLocalSecrets(prev => ({ ...prev, [field]: value }));
    const handleSaveConfig = () => sendCommand?.('UpdateConfig', localConfig);
    const handleSaveSecrets = () => sendCommand?.('UpdateSecrets', localSecrets);
    const handleStop = () => sendCommand?.('EmergencyStop', {});
    const handleResume = () => sendCommand?.('Resume', {});

    return (
        <div className="bg-white/5 border border-white/10 rounded-2xl p-8">
            <div className="flex items-center gap-3 mb-8">
                <div className="p-2.5 bg-white/10 rounded-xl border border-white/10">
                    {activeTab === 'control' ? <Sliders className="w-4 h-4 text-white" /> : <ShieldCheck className="w-4 h-4 text-white" />}
                </div>
                <div>
                    <h2 className="text-base font-bold text-white tracking-tight">
                        {activeTab === 'control' ? 'Control Hub' : 'API Vault'}
                    </h2>
                    <p className="text-[10px] text-zinc-600 font-bold tracking-widest uppercase mt-0.5">Config & Credentials</p>
                </div>
            </div>

            {/* Tab Switcher */}
            <div className="flex gap-2 mb-8 bg-white/5 p-1 rounded-xl border border-white/10">
                {['control', 'vault'].map(tab => (
                    <button
                        key={tab}
                        onClick={() => setActiveTab(tab)}
                        className={`flex-1 py-2 rounded-lg text-[10px] font-bold uppercase tracking-widest transition-all ${
                            activeTab === tab ? 'bg-white text-black' : 'text-zinc-500 hover:text-white'
                        }`}
                    >
                        {tab === 'control' ? 'Control' : 'API Keys'}
                    </button>
                ))}
            </div>

            {activeTab === 'control' ? (
                <div className="space-y-6">
                    {/* Kill switches */}
                    <div className="grid grid-cols-2 gap-4">
                        <button
                            onClick={handleStop}
                            className="flex flex-col items-center gap-2 p-4 rounded-xl bg-white/5 hover:bg-rose-500/10 border border-white/10 hover:border-rose-500/20 transition-all"
                        >
                            <ShieldAlert className="w-5 h-5 text-rose-500" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-rose-400">Kill Switch</span>
                        </button>
                        <button
                            onClick={handleResume}
                            className="flex flex-col items-center gap-2 p-4 rounded-xl bg-white/5 hover:bg-emerald-500/10 border border-white/10 hover:border-emerald-500/20 transition-all"
                        >
                            <Play className="w-5 h-5 text-emerald-500" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-emerald-400">Resume Bot</span>
                        </button>
                    </div>

                    {/* Risk params */}
                    <div className="space-y-4">
                        <div className="flex items-center gap-2">
                            <Zap className="w-3 h-3 text-zinc-500" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-600">Risk Parameters</span>
                        </div>
                        <div className="grid grid-cols-2 gap-4">
                            <InputField label="Margin Low" value={localConfig.margin_threshold_low} onChange={v => handleChange('margin_threshold_low', v)} />
                            <InputField label="Margin High" value={localConfig.margin_threshold_high} onChange={v => handleChange('margin_threshold_high', v)} />
                            <InputField label="Target Ratio" value={localConfig.target_margin_ratio} onChange={v => handleChange('target_margin_ratio', v)} />
                            <InputField label="Concentration" value={localConfig.concentration_threshold} onChange={v => handleChange('concentration_threshold', v)} />
                        </div>
                    </div>

                    <button onClick={handleSaveConfig} className="w-full py-3 rounded-xl bg-white text-black text-xs font-bold uppercase tracking-widest hover:bg-zinc-200 transition-colors flex items-center justify-center gap-2">
                        <Save className="w-3.5 h-3.5" /> Save Config
                    </button>

                    <div className="pt-4 border-t border-white/10 flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Activity className={`w-3 h-3 ${localConfig.automated_rebalance_enabled ? 'text-white animate-pulse' : 'text-zinc-700'}`} />
                            <span className="text-[10px] font-bold uppercase tracking-tighter text-zinc-600">Auto-Rebalance</span>
                        </div>
                        <span className={`px-2.5 py-1 rounded-md text-[10px] font-bold uppercase border ${
                            localConfig.automated_rebalance_enabled
                                ? 'bg-white/10 text-white border-white/20'
                                : 'bg-white/5 text-zinc-600 border-white/10'
                        }`}>
                            {localConfig.automated_rebalance_enabled ? 'Active' : 'Standby'}
                        </span>
                    </div>
                </div>
            ) : (
                <div className="space-y-6 max-h-[500px] overflow-y-auto pr-1">
                    <VaultSection title="Binance">
                        <InputField label="API Key" value={localSecrets.binance_key} onChange={v => handleSecretChange('binance_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.binance_secret} onChange={v => handleSecretChange('binance_secret', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="Bybit">
                        <InputField label="API Key" value={localSecrets.bybit_key} onChange={v => handleSecretChange('bybit_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.bybit_secret} onChange={v => handleSecretChange('bybit_secret', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="Bitget">
                        <InputField label="API Key" value={localSecrets.bitget_key} onChange={v => handleSecretChange('bitget_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.bitget_secret} onChange={v => handleSecretChange('bitget_secret', v)} isSecret />
                        <InputField label="Passphrase" value={localSecrets.bitget_passphrase} onChange={v => handleSecretChange('bitget_passphrase', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="Gate.io">
                        <InputField label="API Key" value={localSecrets.gate_key} onChange={v => handleSecretChange('gate_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.gate_secret} onChange={v => handleSecretChange('gate_secret', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="Bitmart">
                        <InputField label="API Key" value={localSecrets.bitmart_key} onChange={v => handleSecretChange('bitmart_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.bitmart_secret} onChange={v => handleSecretChange('bitmart_secret', v)} isSecret />
                        <InputField label="Memo" value={localSecrets.bitmart_memo} onChange={v => handleSecretChange('bitmart_memo', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="Kraken">
                        <InputField label="API Key" value={localSecrets.kraken_key} onChange={v => handleSecretChange('kraken_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.kraken_secret} onChange={v => handleSecretChange('kraken_secret', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="MEXC">
                        <InputField label="API Key" value={localSecrets.mexc_key} onChange={v => handleSecretChange('mexc_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.mexc_secret} onChange={v => handleSecretChange('mexc_secret', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="OKX">
                        <InputField label="API Key" value={localSecrets.okx_key} onChange={v => handleSecretChange('okx_key', v)} isSecret />
                        <InputField label="Secret" value={localSecrets.okx_secret} onChange={v => handleSecretChange('okx_secret', v)} isSecret />
                        <InputField label="Passphrase" value={localSecrets.okx_passphrase} onChange={v => handleSecretChange('okx_passphrase', v)} isSecret />
                    </VaultSection>
                    <VaultSection title="DEX — Hyperliquid / Aster / Lighter">
                        <p className="text-[11px] text-zinc-500 leading-relaxed">
                            These connectors read public orderbook APIs — no API keys needed for price feeds.
                            Trading execution requires a wallet private key (not yet implemented).
                        </p>
                    </VaultSection>
                    <VaultSection title="Telegram">
                        <InputField label="Bot Token" value={localSecrets.telegram_token} onChange={v => handleSecretChange('telegram_token', v)} isSecret />
                        <InputField label="Chat ID" value={localSecrets.telegram_chat_id} onChange={v => handleSecretChange('telegram_chat_id', v)} />
                    </VaultSection>

                    <button onClick={handleSaveSecrets} className="w-full py-3 rounded-xl bg-white text-black text-xs font-bold uppercase tracking-widest hover:bg-zinc-200 transition-colors flex items-center justify-center gap-2 sticky bottom-0">
                        <ShieldCheck className="w-3.5 h-3.5" /> Save Credentials
                    </button>
                </div>
            )}
        </div>
    );
};

const VaultSection = ({ title, children }) => (
    <div className="space-y-3">
        <div className="text-[10px] font-bold uppercase tracking-widest text-zinc-600 border-b border-white/10 pb-2">{title}</div>
        <div className="space-y-3">{children}</div>
    </div>
);

const InputField = ({ label, value, onChange, isSecret = false }) => (
    <div className="space-y-1">
        <label className="text-[10px] font-bold uppercase tracking-widest text-zinc-600">{label}</label>
        <input
            type={isSecret ? 'password' : 'text'}
            value={value || ''}
            onChange={e => onChange(e.target.value)}
            className="w-full bg-black border border-white/10 rounded-lg px-3 py-2.5 text-white text-sm focus:outline-none focus:border-white/30 transition-colors font-mono placeholder-zinc-700"
        />
    </div>
);

export default SettingsPanel;
