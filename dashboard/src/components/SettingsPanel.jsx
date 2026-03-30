import React, { useState, useEffect } from 'react';
import { Settings, ShieldAlert, ShieldCheck, Play, Pause, Save, Zap, Sliders, Activity } from 'lucide-react';
import { motion } from 'framer-motion';

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
        mexc_key: "", mexc_secret: "",
        okx_key: "", okx_secret: "", okx_passphrase: "",
        telegram_token: "", telegram_chat_id: ""
    });

    useEffect(() => {
        if (config) setLocalConfig(config);
    }, [config]);

    useEffect(() => {
        if (secrets) setLocalSecrets(secrets);
    }, [secrets]);

    const handleChange = (field, value) => {
        setLocalConfig(prev => ({ ...prev, [field]: value }));
    };

    const handleSecretChange = (field, value) => {
        setLocalSecrets(prev => ({ ...prev, [field]: value }));
    };

    const handleSaveConfig = () => {
        sendCommand('UpdateConfig', localConfig);
    };

    const handleSaveSecrets = () => {
        sendCommand('UpdateSecrets', localSecrets);
    };

    const handleStop = () => sendCommand('EmergencyStop', {});
    const handleResume = () => sendCommand('Resume', {});

    return (
        <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            className="relative"
        >
            <div className="glass-morphism rounded-[2.5rem] p-8 border border-slate-800/50 shadow-2xl relative overflow-hidden group">
                <div className="absolute -top-12 -right-12 w-32 h-32 bg-indigo-500/10 rounded-full blur-[60px] group-hover:bg-indigo-500/20 transition-all duration-700" />

                <div className="flex items-center justify-between mb-8">
                    <div className="flex items-center gap-3">
                        <div className="p-3 bg-indigo-500/10 rounded-2xl border border-indigo-500/20">
                            {activeTab === 'control' ? <Sliders className="w-5 h-5 text-indigo-400" /> : <ShieldCheck className="w-5 h-5 text-fuchsia-400" />}
                        </div>
                        <div>
                            <h2 className="text-xl font-black text-white tracking-tight uppercase leading-none">
                                {activeTab === 'control' ? 'Control Hub' : 'Security Vault'}
                            </h2>
                            <p className="text-[9px] text-slate-500 font-bold tracking-widest uppercase mt-1">Config & Secrets</p>
                        </div>
                    </div>
                </div>

                {/* Tab Switcher */}
                <div className="flex gap-2 mb-8 bg-slate-950/50 p-1 rounded-2xl border border-slate-800/50">
                    <button
                        onClick={() => setActiveTab('control')}
                        className={`flex-1 py-2 rounded-xl text-[10px] font-black uppercase tracking-widest transition-all ${activeTab === 'control' ? 'bg-indigo-500 text-white shadow-lg' : 'text-slate-500 hover:text-slate-300'}`}
                    >
                        Control
                    </button>
                    <button
                        onClick={() => setActiveTab('vault')}
                        className={`flex-1 py-2 rounded-xl text-[10px] font-black uppercase tracking-widest transition-all ${activeTab === 'vault' ? 'bg-fuchsia-500 text-white shadow-lg' : 'text-slate-500 hover:text-slate-300'}`}
                    >
                        Vault
                    </button>
                </div>

                {activeTab === 'control' ? (
                    <div className="space-y-6">
                        {/* Kill Switches */}
                        <div className="grid grid-cols-2 gap-4">
                            <button
                                onClick={handleStop}
                                className="flex flex-col items-center justify-center gap-2 p-4 rounded-2xl bg-rose-500/10 hover:bg-rose-500/20 border border-rose-500/20 transition-all group/kill"
                            >
                                <ShieldAlert className="w-6 h-6 text-rose-500 group-hover/kill:scale-110 transition-transform" />
                                <span className="text-[10px] font-black uppercase tracking-widest text-rose-400">Kill Switch</span>
                            </button>
                            <button
                                onClick={handleResume}
                                className="flex flex-col items-center justify-center gap-2 p-4 rounded-2xl bg-emerald-500/10 hover:bg-emerald-500/20 border border-emerald-500/20 transition-all group/resume"
                            >
                                <Play className="w-6 h-6 text-emerald-500 group-hover/resume:scale-110 transition-transform" />
                                <span className="text-[10px] font-black uppercase tracking-widest text-emerald-400">Resume Bot</span>
                            </button>
                        </div>

                        <div className="space-y-4">
                            <div className="flex items-center gap-2 px-1">
                                <Zap className="w-3 h-3 text-amber-500" />
                                <span className="text-[10px] font-black uppercase tracking-widest text-slate-500">Risk Parameters</span>
                            </div>
                            <div className="grid grid-cols-2 gap-4">
                                <InputField label="Margin Low" value={localConfig.margin_threshold_low} onChange={(val) => handleChange('margin_threshold_low', val)} />
                                <InputField label="Margin High" value={localConfig.margin_threshold_high} onChange={(val) => handleChange('margin_threshold_high', val)} />
                                <InputField label="Target Ratio" value={localConfig.target_margin_ratio} onChange={(val) => handleChange('target_margin_ratio', val)} />
                                <InputField label="Concentration" value={localConfig.concentration_threshold} onChange={(val) => handleChange('concentration_threshold', val)} />
                            </div>
                        </div>

                        <button onClick={handleSaveConfig} className="w-full relative group/save overflow-hidden rounded-2xl">
                            <div className="absolute inset-0 bg-gradient-to-r from-indigo-600 to-violet-700 group-hover:from-indigo-500 group-hover:to-violet-600 transition-all" />
                            <div className="relative py-4 flex items-center justify-center gap-3 text-white font-black uppercase tracking-widest text-xs">
                                <Save className="w-4 h-4" /> Sync To Clusters
                            </div>
                        </button>
                    </div>
                ) : (
                    <div className="space-y-6 max-h-[400px] overflow-y-auto pr-2 custom-scrollbar">
                        <div className="space-y-4">
                            <VaultSection title="Binance" icon={<Zap className="w-3 h-3" />}>
                                <InputField label="API Key" value={localSecrets.binance_key} onChange={(val) => handleSecretChange('binance_key', val)} isSecret />
                                <InputField label="API Secret" value={localSecrets.binance_secret} onChange={(val) => handleSecretChange('binance_secret', val)} isSecret />
                            </VaultSection>

                            <VaultSection title="Bybit" icon={<Zap className="w-3 h-3" />}>
                                <InputField label="API Key" value={localSecrets.bybit_key} onChange={(val) => handleSecretChange('bybit_key', val)} isSecret />
                                <InputField label="API Secret" value={localSecrets.bybit_secret} onChange={(val) => handleSecretChange('bybit_secret', val)} isSecret />
                            </VaultSection>

                            <VaultSection title="Bitget" icon={<Zap className="w-3 h-3" />}>
                                <InputField label="API Key" value={localSecrets.bitget_key} onChange={(val) => handleSecretChange('bitget_key', val)} isSecret />
                                <InputField label="API Secret" value={localSecrets.bitget_secret} onChange={(val) => handleSecretChange('bitget_secret', val)} isSecret />
                                <InputField label="Passphrase" value={localSecrets.bitget_passphrase} onChange={(val) => handleSecretChange('bitget_passphrase', val)} isSecret />
                            </VaultSection>

                            <VaultSection title="Telegram" icon={<Activity className="w-3 h-3" />}>
                                <InputField label="Bot Token" value={localSecrets.telegram_token} onChange={(val) => handleSecretChange('telegram_token', val)} isSecret />
                                <InputField label="Chat ID" value={localSecrets.telegram_chat_id} onChange={(val) => handleSecretChange('telegram_chat_id', val)} isSecret />
                            </VaultSection>
                        </div>

                        <button onClick={handleSaveSecrets} className="w-full relative group/save overflow-hidden rounded-2xl shrink-0">
                            <div className="absolute inset-0 bg-gradient-to-r from-fuchsia-600 to-rose-700 group-hover:from-fuchsia-500 group-hover:to-rose-600 transition-all" />
                            <div className="relative py-4 flex items-center justify-center gap-3 text-white font-black uppercase tracking-widest text-xs">
                                <ShieldCheck className="w-4 h-4" /> Commit Credentials
                            </div>
                        </button>
                    </div>
                )}

                {/* Status Footer */}
                <div className="mt-8 pt-6 border-t border-slate-800/50 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <Activity className={`w-3 h-3 ${localConfig.automated_rebalance_enabled ? 'text-emerald-500 animate-pulse' : 'text-slate-600'}`} />
                        <span className="text-[10px] font-black uppercase tracking-tighter text-slate-500">Auto-Rebalance</span>
                    </div>
                    <div className={`px-3 py-1 rounded-lg text-[10px] font-black uppercase border ${localConfig.automated_rebalance_enabled ? 'bg-emerald-500/10 text-emerald-500 border-emerald-500/20' : 'bg-slate-800/50 text-slate-600 border-slate-700/50'
                        }`}>
                        {localConfig.automated_rebalance_enabled ? 'Active' : 'Standby'}
                    </div>
                </div>
            </div>
        </motion.div>
    );
};

const VaultSection = ({ title, icon, children }) => (
    <div className="space-y-3">
        <div className="flex items-center gap-2 px-1">
            <div className="text-fuchsia-500 opacity-50">{icon}</div>
            <span className="text-[10px] font-black uppercase tracking-widest text-slate-500">{title} Credentials</span>
        </div>
        <div className="grid grid-cols-1 gap-3">
            {children}
        </div>
    </div>
);

const InputField = ({ label, value, onChange, isSecret = false }) => (
    <div className="space-y-1.5">
        <label className="text-[9px] font-black uppercase tracking-tighter text-slate-600 ml-1">{label}</label>
        <div className="relative group/input">
            <input
                type={isSecret ? "password" : "text"}
                value={value || ""}
                onChange={(e) => onChange(e.target.value)}
                placeholder={isSecret && value?.startsWith("****") ? "Masked (Update to change)" : ""}
                className="w-full bg-slate-950/80 border border-slate-800 rounded-xl px-4 py-3 text-white text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500/50 transition-all font-mono shadow-inner group-hover/input:border-slate-700"
            />
            {isSecret && value?.startsWith("****") && (
                <div className="absolute right-3 top-1/2 -translate-y-1/2">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-500/40" />
                </div>
            )}
        </div>
    </div>
);

export default SettingsPanel;
