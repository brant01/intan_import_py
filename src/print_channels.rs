

fn print_all_channel_names(result_out: &HashMap<String, DataType>) {
    if let Some(amplifier_channels) = result_out.get("amplifier_channels") {
        print_names_in_group(amplifier_channels);
    }

    if let Some(dc_amplifier_channels) = result_out.get("dc_amplifier_channels") {
        print_names_in_group(dc_amplifier_channels);
    }

    if let Some(stim_channels) = result_out.get("stim_channels") {
        print_names_in_group(stim_channels);
    }

    if let Some(amp_settle_channels) = result_out.get("amp_settle_channels") {
        print_names_in_group(amp_settle_channels);
    }

    if let Some(charge_recovery_channels) = result_out.get("charge_recovery_channels") {
        print_names_in_group(charge_recovery_channels);
    }

    if let Some(compliance_limit_channels) = result_out.get("compliance_limit_channels") {
        print_names_in_group(compliance_limit_channels);
    }

    if let Some(board_adc_channels) = result_out.get("board_adc_channels") {
        print_names_in_group(board_adc_channels);
    }

    if let Some(board_dac_channels) = result_out.get("board_dac_channels") {
        print_names_in_group(board_dac_channels);
    }

    if let Some(board_dig_in_channels) = result_out.get("board_dig_in_channels") {
        print_names_in_group(board_dig_in_channels);
    }

    if let Some(board_dig_out_channels) = result_out.get("board_dig_out_channels") {
        print_names_in_group(board_dig_out_channels);
    }
}

fn print_names_in_group(channels: &DataType) {
    if let DataType::VecChannel(channel_vec) = channels {
        for channel in channel_vec {
            if let Some(DataType::String(name)) = channel.get("name") {
                println!("{}", name);
            }
        }
    }
}
