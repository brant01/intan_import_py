


// Standard library imports
use std::collections::HashMap;
use::std::fmt;
use std::fs::{File, metadata};
use std::f64::consts::PI;
use std::io::{Read, Result, Seek, SeekFrom, self};
use std::io::Error as IOError;
use std::time::Instant;

// External crates
use byteorder::{ByteOrder, ReadBytesExt, LittleEndian};
use ndarray::{Array1, Array2, ArrayView1, ArrayView2, s, Axis};
//use plotters::prelude::*;

// Local modules
// use crate::your_module;

#[derive(Debug, Clone)]
pub enum DataType {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    HashMap(HashMap<String, DataType>),
    VecInt(Vec<i32>),
    VecChannel(Vec<HashMap<String, DataType>>),
    Array(Arrays),
    None,
}

#[derive(Debug, Clone)]
pub enum Arrays {
    ArrayOne(Array1<i32>),
    ArrayTwo(Array2<i32>),
    ArrayTwoBool(Array2<bool>),
}

enum UnknownChannelTypeError {
    AuxInputSignals,
    VddSignals,
    UnknownChannelType,
    IoError
}

impl From<std::io::Error> for UnknownChannelTypeError {
    fn from(_: std::io::Error) -> Self {
        UnknownChannelTypeError::IoError
    }
}

impl fmt::Display for UnknownChannelTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UnknownChannelTypeError::AuxInputSignals => write!(f, "Error: AuxInputSignals"),
            UnknownChannelTypeError::VddSignals => write!(f, "Error: VddSignals"),
            UnknownChannelTypeError::UnknownChannelType => write!(f, "Error: UnknownChannelType"),
            UnknownChannelTypeError::IoError => write!(f, "Error: IoError"),
        }
    }
}

pub fn load_file(file_path: &str) -> std::result::Result<(HashMap<String, DataType>, bool), Box<dyn std::error::Error>> {
    // Start timing
    let tic = Instant::now();

    //open file
    let mut fid: File = File::open(file_path).unwrap();


    // read file header
    let mut header: HashMap<String, DataType> = read_header(&mut fid)?;

    // Calculate how much data is present and summarize to console
    let (data_present, filesize, num_blocks, num_samples) = 
            calculate_data_size(&mut header, &file_path, &mut fid).expect("Error");

    // if .rhd file contains data, read all present data blocks into 'data'
    // dict, and verify the amout of data read.
    let mut data: HashMap<String, Arrays> = HashMap::new();
    if data_present {
        data = read_all_data_blocks(&mut header, num_samples, num_blocks, &mut fid)?;
        //let position = fid.seek(SeekFrom::Current(0))?;
        check_end_of_file(filesize, &mut fid)?;
    }

    // Save information in 'header' to 'result_out' HashMap
    let mut result_out: HashMap<String, DataType> = HashMap::new();
    header_to_result(&header, &mut result_out);

    // If .rhd file contains data, parse data into readable forms and, if
    // necessary, apply the same notch filter that was active during recording.
    if data_present {
        parse_data(&mut header, &mut data);
        apply_notch_filter(&mut header, &mut data);

        // Save recorded data in 'data' to 'result_out' HashMap.
        data_to_result(&header, &mut data, &mut result_out);
    }
    // Otherwise (.rhd file is just a header for One File Per Signal Type or
    // One File Per Channel data formats, in which actual data is saved in
    // separate .dat files), just return data as an empty HashMap.
    /* 
    else {
        data = HashMap::new();
    }
    */

    // Report how long read took.
    println!("Done! Elapsed time: {:.1} seconds", tic.elapsed().as_secs_f64());

    //return the data
    Ok((result_out, data_present))


}


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

fn find_channel_in_group(channel_name: &str, signal_group: &Vec<HashMap<String, DataType>>) -> (bool, usize) {
    for (count, this_channel) in signal_group.iter().enumerate() {
        if let Some(DataType::String(custom_channel_name)) = this_channel.get("custom_channel_name") {
            if custom_channel_name == channel_name {
                return (true, count);
            }
        }
    }
    (false, 0)
}

fn find_channel_in_header(channel_name: &str, header: &HashMap<String, DataType>) -> (bool, String, usize) {
    let mut signal_group_name = String::new();
    let mut channel_found = false;
    let mut channel_index = 0;

    let groups = vec![
        "amplifier_channels",
        "dc_amplifier_channels",
        "stim_channels",
        "amp_settle_channels",
        "charge_recovery_channels",
        "compliance_limit_channels",
        "board_adc_channels",
        "board_dac_channels",
        "board_dig_in_channels",
        "board_dig_out_channels",
    ];

    for group in groups {
        if let Some(DataType::VecChannel(signal_group)) = header.get(group) {
            let (found, index) = find_channel_in_group(channel_name, signal_group);
            if found {
                channel_found = true;
                channel_index = index;
                signal_group_name = group.to_string();
                break;
            }
        }
    }

    if channel_found {
        return (true, signal_group_name, channel_index);
    }

    (false, signal_group_name, channel_index)
}

fn read_header(fid: &mut File) -> std::result::Result<HashMap<String, DataType>, std::io::Error> {
    
    let mut header: HashMap<String, DataType> = HashMap::new();

    check_magic_number(fid)?;
    
    read_version_number(fid, &mut header)?;
    set_num_samples_per_data_block(&mut header);

    read_sample_rate(fid, &mut header)?;
    read_freq_settings(fid, &mut header)?;

    read_notch_filter_frequency(fid, &mut header)?;
    read_impedance_test_frequencies(fid, &mut header)?;
    read_amp_settle_mode(fid, &mut header)?;
    read_charge_recovery_mode(fid, &mut header)?;

    create_frequency_parameters(&mut header)?;

    read_stim_step_size(fid, &mut header)?;
    read_recovery_current_limit(fid, &mut header)?;
    read_recovery_target_voltage(fid, &mut header)?;

    read_notes(fid, &mut header)?;
    read_dc_amp_saved(fid, &mut header)?;
    read_eval_board_mode(fid, &mut header)?;
    read_reference_channel(fid, &mut header)?;


    initialize_channels(&mut header)?;
    read_signal_summary(fid, &mut header)?; 

    Ok(header)
}

fn check_magic_number(fid: &mut File) -> Result<()> {
    let magic_number: u32 = fid.read_u32::<LittleEndian>()?;
    if magic_number != 0xd69127ac {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unrecognized file type."));
    }
    Ok(())
}

fn read_version_number(fid: &mut File, header: &mut HashMap<String, DataType>) -> std::io::Result<()> {
    let mut version_bytes = [0; 4];
    fid.read_exact(&mut version_bytes)?;

    let major = i16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    let minor = i16::from_le_bytes([version_bytes[2], version_bytes[3]]);

    let mut version = HashMap::new();
    version.insert("major".to_string(), DataType::Int(major as i32));
    version.insert("minor".to_string(), DataType::Int(minor as i32));

    header.insert("version".to_string(), DataType::HashMap(version));

    println!("\nReading Intan Technologies RHS Data File, Version {}.{}\n", major, minor);

    Ok(())
}

fn set_num_samples_per_data_block(header: &mut HashMap<String, DataType>) -> () {
    header.insert("num_samples_per_data_block".to_string(), DataType::Int(128));
}

fn read_sample_rate(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("sample_rate".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_freq_settings(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("dsp_enabled".to_string(), DataType::Int(fid.read_i16::<LittleEndian>()? as i32));
    header.insert("actual_dsp_cutoff_frequency".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert( "actual_lower_bandwidth".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("actual_lower_settle_bandwidth".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("actual_upper_bandwidth".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert( "desired_dsp_cutoff_frequency".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("desired_lower_bandwidth".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("desired_lower_settle_bandwidth".to_string(),DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("desired_upper_bandwidth".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_notch_filter_frequency(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    let notch_filter_mode: i32 = fid.read_i16::<LittleEndian>()? as i32;
    //file_data.insert("notch_filter_mode".to_string(), DataType::Int(notch_filter_mode));
    
    match notch_filter_mode {
        1 => header.insert("notch_filter_frequency".to_string(), DataType::Int(50)),
        2 => header.insert("notch_filter_frequency".to_string(), DataType::Int(60)),
        _ => header.insert("notch_filter_frequency".to_string(), DataType::None),
    };
    Ok(())
}

fn read_impedance_test_frequencies(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("desired_impedance_test_frequency".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    header.insert("actual_impedance_test_frequency".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_amp_settle_mode(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("amp_settle_mode".to_string(), DataType::Int(fid.read_i16::<LittleEndian>()? as i32));
    Ok(())
}

fn read_charge_recovery_mode(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("charge_recovery_mode".to_string(), DataType::Int(fid.read_i16::<LittleEndian>()? as i32));
    Ok(())
}


fn create_frequency_parameters(header: &mut HashMap<String, DataType>) -> Result<()> {
    let mut freq: HashMap<String, DataType> = HashMap::new();
    freq.insert("amplifier_sample_rate".to_string(), header.get("sample_rate").unwrap().clone());
    freq.insert("board_adc_sample_rate".to_string(), header.get("sample_rate").unwrap().clone());
    freq.insert("board_dig_in_sample_rate".to_string(), header.get("sample_rate").unwrap().clone());
    
    let keys: Vec<&str> = vec![
        "desired_dsp_cutoff_frequency",
        "actual_dsp_cutoff_frequency",
        "dsp_enabled",
        "desired_lower_bandwidth",
        "desired_lower_settle_bandwidth",
        "actual_lower_bandwidth",
        "actual_lower_settle_bandwidth",
        "desired_upper_bandwidth",
        "actual_upper_bandwidth",
        "notch_filter_frequency",
        "desired_impedance_test_frequency",
        "actual_impedance_test_frequency",
    ];

    copy_from_header(header, &mut freq, keys)?;

    header.insert("frequency_parameters".to_string(), DataType::HashMap(freq));
    Ok(())
}

fn copy_from_header(header: &mut HashMap<String, DataType>, freq: &mut HashMap<String, DataType>, keys: Vec<&str>) -> Result<()> {
        for key in keys {
            freq.insert(key.to_string(), header.get(key).unwrap().clone());
        }
    Ok(())
}

fn read_stim_step_size(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("stim_step_size".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_recovery_current_limit(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("recovery_current_limit".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_recovery_target_voltage(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("recovery_target_voltage".to_string(), DataType::Float(fid.read_f32::<LittleEndian>()?));
    Ok(())
}

fn read_notes(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    
    let mut notes: HashMap<String, DataType> = HashMap::new();

    notes.insert("note1".to_string(), DataType::String(read_qstring(fid)?));
    notes.insert("note2".to_string(), DataType::String(read_qstring(fid)?));
    notes.insert("note3".to_string(), DataType::String(read_qstring(fid)?));

    header.insert("notes".to_string(), DataType::HashMap(notes));
    
    Ok(())
}


fn read_dc_amp_saved(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("dc_amplifier_data_saved".to_string(), DataType::Int(fid.read_i16::<LittleEndian>()? as i32));
    Ok(())
}

fn read_eval_board_mode(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("eval_board_mode".to_string(), DataType::Int(fid.read_i16::<LittleEndian>()? as i32));

    Ok(())
}

fn read_reference_channel(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("reference_channel".to_string(), DataType::String(read_qstring(fid)?));
    Ok(())
}

fn initialize_channels(header: &mut HashMap<String, DataType>) -> Result<()> {
    header.insert("spike_triggers".to_string(), DataType::VecChannel(Vec::new()));
    header.insert("amplifier_channels".to_string(), DataType::VecChannel(Vec::new()));
    header.insert("board_adc_channels".to_string(), DataType::VecChannel(Vec::new()));
    header.insert("board_dac_channels".to_string(), DataType::VecChannel(Vec::new()));
    header.insert("board_dig_in_channels".to_string(), DataType::VecChannel(Vec::new()));
    header.insert("board_dig_out_channels".to_string(), DataType::VecChannel(Vec::new()));
    Ok(())

}

fn read_signal_summary(fid: &mut File, header: &mut HashMap<String, DataType>) -> Result<()> {
    let mut buffer = [0; 2];
    fid.read_exact(&mut buffer)?;
    let number_of_signal_groups: i16 = i16::from_le_bytes(buffer);

    for signal_group in 1..=number_of_signal_groups {
        add_signal_group_information(header, fid, signal_group)?;
    }
    add_num_channels(header);
    print_header_summary(header);

    Ok(())
}

fn add_signal_group_information(header: &mut HashMap<String, DataType>, fid: &mut File, signal_group:i16) -> Result<()> {
    let signal_group_name: String = read_qstring(fid)?;
    let signal_group_prefix: String = read_qstring(fid)?;

    let mut buffer = [0; 6];
    fid.read_exact(&mut buffer)?;
    let (signal_group_enabled, signal_group_num_channels, _) = (i16::from_le_bytes([buffer[0], buffer[1]]),
                                                                        i16::from_le_bytes([buffer[2], buffer[3]]),
                                                                        i16::from_le_bytes([buffer[4], buffer[5]]));
    if signal_group_num_channels > 0 && signal_group_enabled > 0 {
        for _ in 0..signal_group_num_channels {
            add_channel_information(header, fid, &signal_group_name, &signal_group_prefix, signal_group)?;
        }
    }

    Ok(())
}

fn add_channel_information(header: &mut HashMap<String, DataType>, fid: &mut File, signal_group_name: &str, signal_group_prefix: &str, signal_group: i16) -> std::result::Result<(), std::io::Error> {
    let (mut new_channel, mut new_trigger_channel, channel_enabled, signal_type) = read_new_channel(fid, signal_group_name, signal_group_prefix, signal_group)?;
    append_new_channel(header, &mut new_channel, &mut new_trigger_channel, channel_enabled, signal_type)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn read_new_channel(fid: &mut File, signal_group_name: &str, signal_group_prefix: &str, signal_group: i16) -> Result<(HashMap<String, DataType>, HashMap<String, DataType>, i16, i16)> {
    let mut new_channel = HashMap::new();
    new_channel.insert("port_name".to_string(), DataType::String(signal_group_name.to_string()));
    new_channel.insert("port_prefix".to_string(), DataType::String(signal_group_prefix.to_string()));
    new_channel.insert("port_number".to_string(), DataType::Int(signal_group as i32));

    new_channel.insert("native_channel_name".to_string(), DataType::String(read_qstring(fid)?));
    new_channel.insert("custom_channel_name".to_string(), DataType::String(read_qstring(fid)?));

    let mut buffer = [0; 14];
    fid.read_exact(&mut buffer)?;
    let (native_order, custom_order, signal_type, channel_enabled, chip_channel, _, board_stream) = (
        i16::from_le_bytes([buffer[0], buffer[1]]),
        i16::from_le_bytes([buffer[2], buffer[3]]),
        i16::from_le_bytes([buffer[4], buffer[5]]),
        i16::from_le_bytes([buffer[6], buffer[7]]),
        i16::from_le_bytes([buffer[8], buffer[9]]),
        i16::from_le_bytes([buffer[10], buffer[11]]),
        i16::from_le_bytes([buffer[12], buffer[13]]),
    );

    new_channel.insert("native_order".to_string(), DataType::Int(native_order as i32));
    new_channel.insert("custom_order".to_string(), DataType::Int(custom_order as i32));
    new_channel.insert("chip_channel".to_string(), DataType::Int(chip_channel as i32));
    new_channel.insert("board_stream".to_string(), DataType::Int(board_stream as i32));

    let mut new_trigger_channel = HashMap::new();
    let mut buffer = [0; 8];
    fid.read_exact(&mut buffer)?;
    let (voltage_trigger_mode, voltage_threshold, digital_trigger_channel, digital_edge_polarity) = (
        i16::from_le_bytes([buffer[0], buffer[1]]),
        i16::from_le_bytes([buffer[2], buffer[3]]),
        i16::from_le_bytes([buffer[4], buffer[5]]),
        i16::from_le_bytes([buffer[6], buffer[7]]),
    );

    new_trigger_channel.insert("voltage_trigger_mode".to_string(), DataType::Int(voltage_trigger_mode as i32));
    new_trigger_channel.insert("voltage_threshold".to_string(), DataType::Int(voltage_threshold as i32));
    new_trigger_channel.insert("digital_trigger_channel".to_string(), DataType::Int(digital_trigger_channel as i32));
    new_trigger_channel.insert("digital_edge_polarity".to_string(), DataType::Int(digital_edge_polarity as i32));

    let mut buffer = [0; 8];
    fid.read_exact(&mut buffer)?;
    let (electrode_impedance_magnitude, electrode_impedance_phase) = (
        f32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]),
        f32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]),
    );

    new_channel.insert("electrode_impedance_magnitude".to_string(), DataType::Float(electrode_impedance_magnitude));
    new_channel.insert("electrode_impedance_phase".to_string(), DataType::Float(electrode_impedance_phase));

    Ok((new_channel, new_trigger_channel, channel_enabled, signal_type))
}


fn append_new_channel(header: &mut HashMap<String, DataType>, new_channel: &mut HashMap<String, DataType>, new_trigger_channel: &mut HashMap<String, DataType>, channel_enabled: i16, signal_type: i16) -> std::result::Result<(), UnknownChannelTypeError> {
    if channel_enabled == 0 {
        return Ok(());
    }

    match signal_type {
        0 => {
            if let DataType::VecChannel(ref mut vec) = header.get_mut("amplifier_channels").unwrap() {
                vec.push(new_channel.clone());
            }
            if let DataType::VecChannel(ref mut vec) = header.get_mut("spike_triggers").unwrap() {
                vec.push(new_trigger_channel.clone());
            }
        },
        1 => return Err(UnknownChannelTypeError::AuxInputSignals),
        2 => return Err(UnknownChannelTypeError::VddSignals),
        3 => {
            if let DataType::VecChannel(ref mut vec) = header.get_mut("board_adc_channels").unwrap() {
                vec.push(new_channel.clone());
            }
        },
        4 => {
            if let DataType::VecChannel(ref mut vec) = header.get_mut("board_dac_channels").unwrap() {
                vec.push(new_channel.clone());
            }
        },
        5 => {
            if let DataType::VecChannel(ref mut vec) = header.get_mut("board_dig_in_channels").unwrap() {
                vec.push(new_channel.clone());
            }
        },
        6 => {
            if let DataType::VecChannel(ref mut vec) = header.get_mut("board_dig_out_channels").unwrap() {
                vec.push(new_channel.clone());
            }
        },
        _ => return Err(UnknownChannelTypeError::UnknownChannelType),
    }

    Ok(())
}


fn add_num_channels(header: &mut HashMap<String, DataType>) {
    if let DataType::VecChannel(ref vec) = header.get("amplifier_channels").unwrap() {
        header.insert("num_amplifier_channels".to_string(), DataType::Int(vec.len() as i32));
    }
    if let DataType::VecChannel(ref vec) = header.get("board_adc_channels").unwrap() {
        header.insert("num_board_adc_channels".to_string(), DataType::Int(vec.len() as i32));
    }
    if let DataType::VecChannel(ref vec) = header.get("board_dac_channels").unwrap() {
        header.insert("num_board_dac_channels".to_string(), DataType::Int(vec.len() as i32));
    }
    if let DataType::VecChannel(ref vec) = header.get("board_dig_in_channels").unwrap() {
        header.insert("num_board_dig_in_channels".to_string(), DataType::Int(vec.len() as i32));
    }
    if let DataType::VecChannel(ref vec) = header.get("board_dig_out_channels").unwrap() {
        header.insert("num_board_dig_out_channels".to_string(), DataType::Int(vec.len() as i32));
    }
}


fn header_to_result(header: &HashMap<String, DataType>, result_out: &mut HashMap<String, DataType>) {
    let mut stim_parameters = HashMap::new();
    stim_parameters.insert("stim_step_size".to_string(), header["stim_step_size"].clone());
    stim_parameters.insert("charge_recovery_current_limit".to_string(), header["recovery_current_limit"].clone());
    stim_parameters.insert("charge_recovery_target_voltage".to_string(), header["recovery_target_voltage"].clone());
    stim_parameters.insert("amp_settle_mode".to_string(), header["amp_settle_mode"].clone());
    stim_parameters.insert("charge_recovery_mode".to_string(), header["charge_recovery_mode"].clone());
    result_out.insert("stim_parameters".to_string(), DataType::HashMap(stim_parameters));
    
    result_out.insert("notes".to_string(), header["notes"].clone());

    if let DataType::Int(num_amplifier_channels) = header["num_amplifier_channels"] {
        if num_amplifier_channels > 0 {
            result_out.insert("spike_triggers".to_string(), header["spike_triggers"].clone());
            result_out.insert("amplifier_channels".to_string(), header["amplifier_channels"].clone());
        }
    }

    result_out.insert("notes".to_string(), header["notes"].clone());
    result_out.insert("frequency_parameters".to_string(), header["frequency_parameters"].clone());
    result_out.insert("reference_channel".to_string(), header["reference_channel"].clone());

    if let DataType::Int(num_board_adc_channels) = header["num_board_adc_channels"] {
        if num_board_adc_channels > 0 {
            result_out.insert("board_adc_channels".to_string(), header["board_adc_channels"].clone());
        }
    }
    
    if let DataType::Int(num_board_dac_channels) = header["num_board_dac_channels"] {
        if num_board_dac_channels > 0 {
            result_out.insert("board_dac_channels".to_string(), header["board_dac_channels"].clone());
        }
    }

    if let DataType::Int(num_board_dig_in_channels) = header["num_board_dig_in_channels"] {
        if num_board_dig_in_channels > 0 {
            result_out.insert("board_dig_in_channels".to_string(), header["board_dig_in_channels"].clone());
        }
    }

    if let DataType::Int(num_board_dig_out_channels) = header["num_board_dig_out_channels"] {
        if num_board_dig_out_channels > 0 {
            result_out.insert("board_dig_out_channels".to_string(), header["board_dig_out_channels"].clone());
        }
    }
}


fn print_header_summary(header: &HashMap<String, DataType>) {
    let num_amplifier_channels = match header.get("num_amplifier_channels") {
        Some(DataType::Int(n)) => *n,
        _ => 0,
    };
    println!("Found {} amplifier channel{}.", num_amplifier_channels, plural(num_amplifier_channels));

    let dc_amplifier_data_saved = match header.get("dc_amplifier_data_saved") {
        Some(DataType::Int(n)) => *n > 0,
        _ => false,
    };
    if dc_amplifier_data_saved {
        println!("Found {} DC amplifier channel{}.", num_amplifier_channels, plural(num_amplifier_channels));
    }

    let num_board_adc_channels = match header.get("num_board_adc_channels") {
        Some(DataType::Int(n)) => *n,
        _ => 0,
    };
    println!("Found {} board ADC channel{}.", num_board_adc_channels, plural(num_board_adc_channels));

    let num_board_dac_channels = match header.get("num_board_dac_channels") {
        Some(DataType::Int(n)) => *n,
        _ => 0,
    };
    println!("Found {} board DAC channel{}.", num_board_dac_channels, plural(num_board_dac_channels));

    let num_board_dig_in_channels = match header.get("num_board_dig_in_channels") {
        Some(DataType::Int(n)) => *n,
        _ => 0,
    };
    println!("Found {} board digital input channel{}.", num_board_dig_in_channels, plural(num_board_dig_in_channels));

    let num_board_dig_out_channels = match header.get("num_board_dig_out_channels") {
        Some(DataType::Int(n)) => *n,
        _ => 0,
    };
    println!("Found {} board digital output channel{}.", num_board_dig_out_channels, plural(num_board_dig_out_channels));

    println!("");
}

fn plural(n: i32) -> &'static str {
    if n != 1 {
        "s"
    } else {
        ""
    }
}

fn get_bytes_per_data_block(header: &HashMap<String, DataType>) -> std::result::Result<usize, Box<dyn std::error::Error>> {
    
    // RHS files always have 128 samples per data block.
    // Use this number along with number of channels to accrue a sum of how
    // many bytes each data block should contain

    let num_samples_per_data_block = 128;

    // Timestamps(one channel always present): start with 4 bytes per sample

    let mut bytes_per_block = bytes_per_signal_type(num_samples_per_data_block, 1, 4);

    // Amplifier data: Add 2 bytes per sample per enabled amplifier channel
    if let DataType::Int(num_amplifier_channels) = header.get("num_amplifier_channels").unwrap() {
        bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, *num_amplifier_channels as usize, 2);
    }

    // DC Amplifier data (absent if flag was off).
    if let DataType::Int(dc_amplifier_data_saved) = header.get("dc_amplifier_data_saved").unwrap() {
        if *dc_amplifier_data_saved != 0 {
            if let DataType::Int(num_amplifier_channels) = header.get("num_amplifier_channels").unwrap() {
                bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, *num_amplifier_channels as usize, 2);
            }
        }
    }

    // Stimulation data: Add 2 bytes per sample per enabled amplifier channel. 
    if let DataType::Int(num_amplifier_channels) = header.get("num_amplifier_channels").unwrap() {
        bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, *num_amplifier_channels as usize, 2);
    }

    // Analog inputs: Add 2 bytes per sample per enabled analog input channel. 
    if let DataType::Int(num_board_adc_channels) = header.get("num_board_adc_channels").unwrap() {
        bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, *num_board_adc_channels as usize, 2);
    }

    // Analog outputs: Add 2 bytes per sample per enabled analog output channel.
    if let DataType::Int(num_board_dac_channels) = header.get("num_board_dac_channels").unwrap() {
        bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, *num_board_dac_channels as usize, 2);
    }

    // Digital inputs: Add 2 bytes per sample.
    // Note that if at least 1 channel is enabled, a single 16-but sample
    // is saved, with each bit corresponding to an individual channel.
    if let DataType::Int(num_board_dig_in_channels) = header.get("num_board_dig_in_channels").unwrap() {
        if *num_board_dig_in_channels > 0 {
            bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, 1, 2);
        }
    }

    // Digital outputs: Add 2 bytes per sample.
    // Note that if at least 1 channel is enabled, a single 16-bit sample
    // is saved, with each bit corresponding to an individual channel.
    if let DataType::Int(num_board_dig_out_channels) = header.get("num_board_dig_out_channels").unwrap() {
        if *num_board_dig_out_channels > 0 {
            bytes_per_block += bytes_per_signal_type(num_samples_per_data_block, 1, 2);
        }
    }

    Ok(bytes_per_block)
}

fn bytes_per_signal_type(num_samples: usize, num_channels: usize, bytes_per_sample: usize) -> usize {
    num_samples * num_channels * bytes_per_sample
}

fn read_one_data_block(data: &mut HashMap<String, Arrays>, header: &HashMap<String, DataType>, index: &mut u64, fid: &mut File) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let samples_per_block: u64;
    if let DataType::Int(num_samples_per_data_block) = header.get("num_samples_per_data_block").unwrap() {
        samples_per_block = *num_samples_per_data_block as u64;
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_samples_per_data_block is not an integer")));
    }
    read_timestamps(fid, data, *index, samples_per_block)?;
    read_analog_signals(fid, data, *index, samples_per_block, header)?;
    read_digital_signals(fid, data, *index, samples_per_block, header)?;

    Ok(())
}


fn read_timestamps(fid: &mut File, data: &mut HashMap<String, Arrays>, index: u64, num_samples: u64) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = index as usize;
    let end = start + num_samples as usize;

    let mut buffer = vec![0; num_samples as usize * 4];
    fid.read_exact(&mut buffer)?;

    let timestamps: Vec<i32> = buffer.chunks_exact(4).map(|bytes| i32::from_le_bytes(bytes.try_into().unwrap())).collect();

    if let Some(Arrays::ArrayOne(t)) = data.get_mut("t") {
        let mut t_slice = t.slice_mut(s![start..end]);
        t_slice.assign(&ArrayView1::from(&timestamps));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "t key not found in data HashMap")));
    }
    
    Ok(())
}


fn read_analog_signals(fid: &mut File, data: &mut HashMap<String, Arrays>, index: u64, samples_per_block: u64, header: &HashMap<String, DataType>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let num_amplifier_channels = match header.get("num_amplifier_channels") {
        Some(DataType::Int(n)) => *n,
        _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'num_amplifier_channels' is not an Int in 'header'"))),
    };

    read_analog_signal_type(fid,
                            data.get_mut("amplifier_data").ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "'amplifier_data' is not in 'data'"))?,
                            index,
                            samples_per_block,
                            num_amplifier_channels)?;

    if let Some(DataType::Int(n)) = header.get("dc_amplifier_data_saved") {
        if *n > 0 {
            read_analog_signal_type(fid,
                                    data.get_mut("dc_amplifier_data").ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "'dc_amplifier_data' is not in 'data'"))?,
                                    index,
                                    samples_per_block,
                                    num_amplifier_channels)?;
        }
    }

    read_analog_signal_type(fid,
                            data.get_mut("stim_data_raw").ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "'stim_data_raw' is not in 'data'"))?,
                            index,
                            samples_per_block,
                            num_amplifier_channels)?;

    let num_board_adc_channels = match header.get("num_board_adc_channels") {
        Some(DataType::Int(n)) => *n,
        _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'num_board_adc_channels' is not an Int in 'header'"))),
    };

    read_analog_signal_type(fid,
                            data.get_mut("board_adc_data").ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "'board_adc_data' is not in 'data'"))?,
                            index,
                            samples_per_block,
                            num_board_adc_channels)?;

    read_analog_signal_type(fid,
                            data.get_mut("board_dac_data").ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'board_dac_data' is not in 'data'")))?,
                            index,
                            samples_per_block,
                            match header.get("num_board_dac_channels") {
                                Some(DataType::Int(n)) => *n,
                                _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'num_board_dac_channels' is not an Int in 'header'"))),
                            })?;

    Ok(())
}

fn read_analog_signal_type(fid: &mut File, dest: &mut Arrays, start: u64, num_samples: u64, num_channels: i32) -> std::result::Result<(), Box<dyn std::error::Error>> {
    if num_channels < 1 {
        return Ok(());
    }
    let start = start as usize;
    let num_samples = num_samples as usize;
    let num_channels = num_channels as usize;
    let end = start + num_samples;

    let mut buffer = vec![0; num_samples * num_channels * 2];
    fid.read_exact(&mut buffer)?;

    let analog_signals: Vec<u16> = buffer.chunks_exact(2).map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap())).collect();
    let analog_signals_i32: Vec<i32> = analog_signals.iter().map(|&x| x as i32).collect();

    if let Arrays::ArrayTwo(t) = dest {
        let end = std::cmp::min(end, t.len_of(Axis(1)));
        let mut t_slice = t.slice_mut(s![.., start..end]);
        let reshaped_signals = ArrayView2::from_shape((num_channels, num_samples), &analog_signals_i32).unwrap();
        t_slice.assign(&reshaped_signals);
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Expected ArrayTwo")));
    }

    Ok(())
}

fn read_digital_signals(fid: &mut File, data: &mut HashMap<String, Arrays>, index: u64, samples_per_block: u64, header: &HashMap<String, DataType>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let num_board_dig_in_channels = match header.get("num_board_dig_in_channels") {
        Some(DataType::Int(n)) => *n,
        _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'num_board_dig_in_channels' is not an Int in 'header'"))),
    };

    if num_board_dig_in_channels > 0 {
        read_digital_signal_type(fid,
                                 data.get_mut("board_dig_in_raw").ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'board_dig_in_raw' is not in 'data'")))?,
                                 index,
                                 samples_per_block,
                                 num_board_dig_in_channels);
    }

    let num_board_dig_out_channels = match header.get("num_board_dig_out_channels") {
        Some(DataType::Int(n)) => *n,
        _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'num_board_dig_out_channels' is not an Int in 'header'"))),
    };

    if num_board_dig_out_channels > 0 {
        read_digital_signal_type(fid,
                                 data.get_mut("board_dig_out_raw").ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "'board_dig_out_raw' is not in 'data'")))?,
                                 index,
                                 samples_per_block,
                                 num_board_dig_out_channels);
    }

    Ok(())
}


fn read_digital_signal_type(fid: &mut File, dest: &mut Arrays, start: u64, num_samples: u64, num_channels: i32) {
    if num_channels < 1 {
        return;
    }
    let start = start as usize;
    let num_samples = num_samples as usize;
    let num_channels = num_channels as usize;
    let end = start + num_samples;

    let mut buffer = vec![0; num_samples * num_channels * 2];
    fid.read_exact(&mut buffer).unwrap();

    let digital_signals: Vec<i32> = buffer.chunks_exact(2).map(|bytes| LittleEndian::read_u16(bytes) as i32).collect();

    match dest {
        Arrays::ArrayTwo(t) => {
            let mut t_slice = t.slice_mut(s![.., start..end]);
            let num_samples = t_slice.dim().1;
            let reshaped_signals = ArrayView2::from_shape((num_channels, num_samples), &digital_signals[..num_channels * num_samples]).unwrap();
            t_slice.assign(&reshaped_signals);
        },
        _ => eprintln!("Expected ArrayTwo"),
    }

}

fn data_to_result(header: &HashMap<String, DataType>, data: &mut HashMap<String, Arrays>, result_out: &mut HashMap<String, DataType>) {
    result_out.insert("t".to_string(), DataType::Array(data.remove("t").unwrap()));
    result_out.insert("stim_data".to_string(), DataType::Array(data.remove("stim_data").unwrap()));

    if let DataType::Bool(dc_amplifier_data_saved) = header["dc_amplifier_data_saved"] {
        if dc_amplifier_data_saved {
            result_out.insert("dc_amplifier_data".to_string(), DataType::Array(data.remove("dc_amplifier_data").unwrap()));
        }
    }

    if let DataType::Int(num_amplifier_channels) = header["num_amplifier_channels"] {
        if num_amplifier_channels > 0 {
            result_out.insert("compliance_limit_data".to_string(), DataType::Array(data.remove("compliance_limit_data").unwrap()));
            result_out.insert("charge_recovery_data".to_string(), DataType::Array(data.remove("charge_recovery_data").unwrap()));
            result_out.insert("amp_settle_data".to_string(), DataType::Array(data.remove("amp_settle_data").unwrap()));
            result_out.insert("amplifier_data".to_string(), DataType::Array(data.remove("amplifier_data").unwrap()));
        }
    }

    if let DataType::Int(num_board_adc_channels) = header["num_board_adc_channels"] {
        if num_board_adc_channels > 0 {
            result_out.insert("board_adc_data".to_string(), DataType::Array(data.remove("board_adc_data").unwrap()));
        }
    }

    if let DataType::Int(num_board_dac_channels) = header["num_board_dac_channels"] {
        if num_board_dac_channels > 0 {
            result_out.insert("board_dac_data".to_string(), DataType::Array(data.remove("board_dac_data").unwrap()));
        }
    }

    if let DataType::Int(num_board_dig_in_channels) = header["num_board_dig_in_channels"] {
        if num_board_dig_in_channels > 0 {
            result_out.insert("board_dig_in_data".to_string(), DataType::Array(data.remove("board_dig_in_data").unwrap()));
        }
    }

    if let DataType::Int(num_board_dig_out_channels) = header["num_board_dig_out_channels"] {
        if num_board_dig_out_channels > 0 {
            result_out.insert("board_dig_out_data".to_string(), DataType::Array(data.remove("board_dig_out_data").unwrap()));
        }
    }
}


/* 
fn plot_channel(channel_name: &str, result_out: &HashMap<String, DataType>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (channel_found, signal_type, signal_index) = find_channel_in_header(channel_name, result_out);

    if channel_found {
        let root = BitMapBackend::new("plot.png", (640, 480)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(channel_name, ("sans-serif", 50).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(0f32..1f32, 0f32..1f32)?;

        chart.configure_mesh().draw()?;

        let signal_data_name;
        let ylabel;

        match signal_type.as_str() {
            "amplifier_channels" => {
                ylabel = "Voltage (microVolts)";
                signal_data_name = "amplifier_data";
            }
            "dc_amplifier_channels" => {
                ylabel = "Voltage (Volts)";
                signal_data_name = "dc_amplifier_data";
            }
            "stim_channels" => {
                ylabel = "Current (microAmps)";
                signal_data_name = "stim_data";
            }
            "amp_settle_channels" => {
                ylabel = "Amp Settle Events (High or Low)";
                signal_data_name = "amp_settle_data";
            }
            "charge_recovery_channels" => {
                ylabel = "Charge Recovery Events (High or Low)";
                signal_data_name = "charge_recovery_data";
            }
            "compliance_limit_channels" => {
                ylabel = "Compliance Limit Events (High or Low)";
                signal_data_name = "compliance_limit_data";
            }
            "board_adc_channels" => {
                ylabel = "Voltage (Volts)";
                signal_data_name = "board_adc_data";
            }
            "board_dac_channels" => {
                ylabel = "Voltage (Volts)";
                signal_data_name = "board_dac_data";
            }
            "board_dig_in_channels" => {
                ylabel = "Digital In Events (High or Low)";
                signal_data_name = "board_dig_in_data";
            }
            "board_dig_out_channels" => {
                ylabel = "Digital Out Events (High or Low)";
                signal_data_name = "board_dig_out_data";
            }
            _ => return Err(Box::new(RhsError::ChannelNotFoundError)),
        }

        let signal_data = result_out.get(signal_data_name).unwrap();
        let time_data = result_out.get("t").unwrap();

        if let DataType::Array(Arrays::ArrayOne(time_data)) = time_data {
            if let DataType::VecChannel(signal_data) = signal_data {
                if let Some(DataType::Float(signal_value)) = signal_data[signal_index].get("signal_value") {
                    let signal_value_vec: Vec<f32> = vec![*signal_value; time_data.len()];
                    chart.draw_series(LineSeries::new(
                        time_data.iter().zip(signal_value_vec.iter()).map(|(x, y)| (*x as f32, *y as f32)),
                        &RED,
                    ))?;
                }
            }
        }
        chart.configure_series_labels().draw()?;
    } else {
        return Err(Box::new(RhsError::ChannelNotFoundError));
    }

    Ok(())
}
*/

fn read_qstring(fid: &mut File) -> std::result::Result<String, IOError> {
    let length: u32 = fid.read_u32::<LittleEndian>()?;
    
    // if length set to 0xFFFFFFFF, return empty string
    if length == 0xFFFFFFFF {
        return Ok(String::new());
    }

    let current_position = fid.seek(SeekFrom::Current(0))?;
    let file_length = fid.seek(SeekFrom::End(0))?;
    fid.seek(SeekFrom::Start(current_position))?;


    if length as u64 > file_length - current_position + 1 {
        return Err(IOError::new(std::io::ErrorKind::InvalidData, "Length too long."));
    }

    // Convert length from bytes to 16-bit Unicode words.
    let length = (length / 2) as usize;

    let mut data = Vec::new();
    for _ in 0..length {
        let c = fid.read_u16::<LittleEndian>()?;
        data.push(c);
    }

    //let a: String = data.iter().map(|&c| char::from_u32(c as u32).unwrap()).collect();
    let mut a = String::new();
    for &c in &data {
        match char::from_u32(c as u32) {
            Some(ch) => a.push(ch),
            None => a.push_str("None"),
        }
    }

    Ok(a)
}

fn calculate_data_size(header: &mut HashMap<String, DataType>, filename: &str, fid: &mut File) -> std::result::Result<(bool, u64, u64, u64), Box<dyn std::error::Error>> {
    let bytes_per_block = get_bytes_per_data_block(header)?;

    // Determine filesize and if any data is present.
    let metadata = metadata(filename)?;
    let filesize = metadata.len();
    let mut data_present: bool = false;
    let bytes_remaining = filesize - fid.seek(SeekFrom::Current(0))?;
    if bytes_remaining > 0 {
        data_present = true;
    }

    // If the file size is somehow different than expected, raise an error.
    if bytes_remaining % bytes_per_block as u64 != 0 {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Something is wrong with file size : should have a whole number of data blocks")));
    }

    // Calculate how many data blocks are present.
    let num_blocks = bytes_remaining / bytes_per_block as u64;
    
    let num_samples = calculate_num_samples(header, num_blocks)?;

    let sample_rate: f32;
    match header.get("sample_rate") {
        Some(DataType::Float(rate)) => sample_rate = *rate,
        _ => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "sample_rate is not a float"))),
    }

    print_record_time_summary(num_samples, sample_rate, data_present);

    Ok((data_present, filesize, num_blocks, num_samples))
}

fn calculate_num_samples(header: &mut HashMap<String, DataType>, num_data_blocks: u64) -> std::result::Result<u64, Box<dyn std::error::Error>> {
    if let DataType::Int(num_samples_per_data_block) = header.get("num_samples_per_data_block").unwrap() {
        Ok((*num_samples_per_data_block as u64) * num_data_blocks)
    } else {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_samples_per_data_block is not an integer")))
    }
}

fn print_record_time_summary(num_amp_samples: u64, sample_rate: f32, data_present: bool) {
    let record_time = num_amp_samples as f32 / sample_rate;

    if data_present {
        println!("File contains {:.3} seconds of data. Amplifiers were sampled at {:.2} kS/s.", record_time, sample_rate / 1000.0);
    } else {
        println!("Header file contains no data. Amplifiers were sampled at {:.2} kS/s.", sample_rate / 1000.0);
    }
}

fn read_all_data_blocks(header: &mut HashMap<String, DataType>, num_samples: u64, num_blocks: u64, fid: &mut File) -> std::result::Result<HashMap<String, Arrays>, Box<dyn std::error::Error>> {
    let (mut data, mut index) = initialize_memory(header, num_samples)?;
    println!("Reading data from file...");
    let print_step = 10;
    let num_blocks = num_blocks as usize;
    let mut percent_done = print_step;

    for i in 0..num_blocks {
        read_one_data_block(&mut data, header, &mut index, fid)?;
        if let DataType::Int(num_samples_per_data_block) = header.get("num_samples_per_data_block").unwrap() {
            index = advance_index(index, *num_samples_per_data_block as u64);
        } else {
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_samples_per_data_block is not an integer")));
        }
        percent_done = print_progress(i, num_blocks, print_step, percent_done);
    }
    Ok(data)
}


fn initialize_memory(header: &HashMap<String, DataType>, num_samples: u64) -> std::result::Result<(HashMap<String, Arrays>, u64), Box<dyn std::error::Error>> {
    println!("\nAllocating memory for data...");
    let mut data: HashMap<String, Arrays> = HashMap::new();

    // Create zero array for timestamps.
    data.insert("t".to_string(), Arrays::ArrayOne(Array1::zeros(num_samples as usize,)));

    // Create zero array for amplifier data.
    if let DataType::Int(num_amplifier_channels) = header.get("num_amplifier_channels").unwrap() {
        data.insert("amplifier_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_amplifier_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_amplifier_channels is not an integer")));
    }

    // Create zero array for DC amplifier data.
    if let Some(DataType::Int(dc_amplifier_data_saved)) = header.get("dc_amplifier_data_saved") {
        if *dc_amplifier_data_saved == 1 {
            if let Some(DataType::Int(num_amplifier_channels)) = header.get("num_amplifier_channels") {
                data.insert("dc_amplifier_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_amplifier_channels as usize, num_samples as usize))));
            } else {
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_amplifier_channels is not an integer")));
            }
        }
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "dc_amplifier_data_saved is not an integer")));
    }

    // Create zero array for stim data.
    if let DataType::Int(num_amplifier_channels) = header.get("num_amplifier_channels").unwrap() {
        data.insert("stim_data_raw".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_amplifier_channels as usize, num_samples as usize))));
        data.insert("stim_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_amplifier_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_amplifier_channels is not an integer")));
    }

    // Create zero array for board ADC data.
    if let DataType::Int(num_board_adc_channels) = header.get("num_board_adc_channels").unwrap() {
        data.insert("board_adc_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_adc_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_board_adc_channels is not an integer")));
    }

    // Create zero array for board DAC data.
    if let DataType::Int(num_board_dac_channels) = header.get("num_board_dac_channels").unwrap() {
        data.insert("board_dac_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_dac_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_board_dac_channels is not an integer")));
    }

    // Create zero array for digital in data.
    if let DataType::Int(num_board_dig_in_channels) = header.get("num_board_dig_in_channels").unwrap() {
        data.insert("board_dig_in_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_dig_in_channels as usize, num_samples as usize))));
        data.insert("board_dig_in_raw".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_dig_in_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_board_dig_in_channels is not an integer")));
    }

    // Create zero array for digital out data.
    if let DataType::Int(num_board_dig_out_channels) = header.get("num_board_dig_out_channels").unwrap() {
        data.insert("board_dig_out_data".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_dig_out_channels as usize, num_samples as usize))));
        data.insert("board_dig_out_raw".to_string(), Arrays::ArrayTwo(Array2::zeros((*num_board_dig_out_channels as usize, num_samples as usize))));
    } else {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "num_board_dig_out_channels is not an integer")));
    }

    // Set index representing position of data (shared across all signal types
    // for RHS file) to 0
    let index = 0;

    Ok((data, index))
}

fn advance_index(index: u64, samples_per_block: u64) -> u64 {
    // For RHS, all signals sampled at the same sample rate:
    // Index should be incremented by samples_per_block every data block.
    let index = index + samples_per_block;
    index
}

fn check_end_of_file(filesize: u64, fid: &mut File) -> io::Result<()> {
    let current_position = fid.seek(SeekFrom::Current(0))?;
    let bytes_remaining = filesize - current_position;
    if bytes_remaining != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Error: End of file not reached. Current position: {}, File size: {}, Bytes remaining: {}", current_position, filesize, bytes_remaining)));
    }
    Ok(())
}

fn parse_data(header: &mut HashMap<String, DataType>, data: &mut HashMap<String, Arrays>) {
    println!("Parsing data...");
    extract_digital_data(header, data);
    extract_stim_data(data);
    scale_analog_data(header, data);
    scale_timestamps(header, data);
}


fn scale_timestamps(header: &mut HashMap<String, DataType>, data: &mut HashMap<String, Arrays>) {
    // Check for gaps in timestamps.
    if let Some(Arrays::ArrayOne(t)) = data.get_mut("t") {
        let num_gaps = t.windows(2).into_iter().filter(|window| window[1] - window[0] != 1).count();
        if num_gaps == 0 {
            println!("No missing timestamps in data.");
        } else {
            println!("Warning: {} gaps in timestamp data found. Time scale will not be uniform!", num_gaps);
        }

        // Scale time steps (units = seconds).
        if let DataType::Float(sample_rate) = header["sample_rate"] {
            *t = t.mapv(|x| (x as f32 / sample_rate) as i32);
        }
    }
}

fn scale_analog_data(header: &mut HashMap<String, DataType>, data: &mut HashMap<String, Arrays>) {
    // Scale amplifier data (units = microVolts).
    if let Some(Arrays::ArrayTwo(amplifier_data)) = data.get_mut("amplifier_data") {
        amplifier_data.map_inplace(|x| *x = (0.195 * (*x as f32 - 32768.0)) as i32);
    }

    // Scale stim data.
    if let DataType::Float(stim_step_size) = header["stim_step_size"] {
        if let Some(Arrays::ArrayTwo(stim_data)) = data.get_mut("stim_data") {
            stim_data.map_inplace(|x| *x = (*x as f32 * stim_step_size) as i32);
        }
    }

    // Scale DC amplifier data (units = Volts).
    if let DataType::Bool(dc_amplifier_data_saved) = header["dc_amplifier_data_saved"] {
        if dc_amplifier_data_saved {
            if let Some(Arrays::ArrayTwo(dc_amplifier_data)) = data.get_mut("dc_amplifier_data") {
                dc_amplifier_data.map_inplace(|x| *x = (-0.01923 * (*x as f32 - 512.0)) as i32);
            }
        }
    }

    // Scale board ADC data (units = Volts).
    if let Some(Arrays::ArrayTwo(board_adc_data)) = data.get_mut("board_adc_data") {
        board_adc_data.map_inplace(|x| *x = (312.5e-6 * (*x as f32 - 32768.0)) as i32);
    }

    // Scale board DAC data (units = Volts).
    if let Some(Arrays::ArrayTwo(board_dac_data)) = data.get_mut("board_dac_data") {
        board_dac_data.map_inplace(|x| *x = (312.5e-6 * (*x as f32 - 32768.0)) as i32);
    }
}


fn extract_digital_data(header: &mut HashMap<String, DataType>, data: &mut HashMap<String, Arrays>) {
    if let DataType::Int(num_board_dig_in_channels) = header["num_board_dig_in_channels"] {
        if let DataType::VecChannel(board_dig_in_channels) = &header["board_dig_in_channels"] {
            if let Some(Arrays::ArrayTwo(board_dig_in_raw)) = data.remove("board_dig_in_raw") {
                let mut board_dig_in_data = Array2::<i32>::zeros(board_dig_in_raw.dim());
                for i in 0..num_board_dig_in_channels as usize {
                    if let Some(DataType::Int(native_order)) = board_dig_in_channels[i].get("native_order") {
                        let mask = 1 << *native_order as usize;
                        let mapped = board_dig_in_raw.mapv(|x| if x & mask != 0 { 1 } else { 0 });
                        board_dig_in_data.row_mut(i).assign(&mapped.index_axis(Axis(0), 0));
                    }
                }
                data.insert("board_dig_in_data".to_string(), Arrays::ArrayTwo(board_dig_in_data));
            }
        }
    }

    if let DataType::Int(num_board_dig_out_channels) = header["num_board_dig_out_channels"] {
        if let DataType::VecChannel(board_dig_out_channels) = &header["board_dig_out_channels"] {
            if let Some(Arrays::ArrayTwo(board_dig_out_raw)) = data.remove("board_dig_out_raw") {
                let mut board_dig_out_data = Array2::<i32>::zeros(board_dig_out_raw.dim());
                for i in 0..num_board_dig_out_channels as usize {
                    if let Some(DataType::Int(native_order)) = board_dig_out_channels[i].get("native_order") {
                        let mask = 1 << *native_order as usize;
                        let mapped = board_dig_out_raw.mapv(|x| if x & mask != 0 { 1 } else { 0 });
                        board_dig_out_data.row_mut(i).assign(&mapped.index_axis(Axis(0), 0));
                    }
                }
                data.insert("board_dig_out_data".to_string(), Arrays::ArrayTwo(board_dig_out_data));
            }
        }
    }

}

fn extract_stim_data(data: &mut HashMap<String, Arrays>) {
    if let Some(Arrays::ArrayTwo(stim_data_raw)) = data.get_mut("stim_data_raw") {
        // Interpret 2^15 bit (compliance limit) as true or false.
        let compliance_limit_data = stim_data_raw.mapv(|x| (x & 32768) >= 1);

        // Interpret 2^14 bit (charge recovery) as true or false.
        let charge_recovery_data = stim_data_raw.mapv(|x| (x & 16384) >= 1);

        // Interpret 2^13 bit (amp settle) as true or false.
        let amp_settle_data = stim_data_raw.mapv(|x| (x & 8192) >= 1);

        // Interpret 2^8 bit (stim polarity) as +1 for 0_bit or -1 for 1_bit.
        let stim_polarity = stim_data_raw.mapv(|x| 1 - 2 * ((x & 256) >> 8));

        // Get least-significant 8 bits corresponding to the current amplitude.
        let curr_amp = stim_data_raw.mapv(|x| x & 255);

        // Multiply current amplitude by the correct sign.
        let stim_data = &curr_amp * &stim_polarity;

        data.insert("compliance_limit_data".to_string(), Arrays::ArrayTwoBool(compliance_limit_data));
        data.insert("charge_recovery_data".to_string(), Arrays::ArrayTwoBool(charge_recovery_data));
        data.insert("amp_settle_data".to_string(), Arrays::ArrayTwoBool(amp_settle_data));
        data.insert("stim_polarity".to_string(), Arrays::ArrayTwo(stim_polarity.clone()));
        data.insert("stim_data".to_string(), Arrays::ArrayTwo(stim_data));
    }
}

fn apply_notch_filter(header: &mut HashMap<String, DataType>, data: &mut HashMap<String, Arrays>) {
    // If data was not recorded with notch filter turned on, return without
    // applying notch filter. Similarly, if data was recorded from Intan RHX
    // software version 3.0 or later, any active notch filter was already
    // applied to the saved data, so it should not be re-applied.
    if let DataType::Int(notch_filter_frequency) = &header["notch_filter_frequency"] {
        if notch_filter_frequency == &0 {
            return;
        }
        if let DataType::HashMap(version) = &header["version"] {
            if let (DataType::Int(major), DataType::Int(_)) = (&version["major"], &version["minor"]) {
                if major >= &3 {
                    return;
                }
            }
        }
    }

    // Apply notch filter individually to each channel in order
    println!("Applying notch filter...");
    let print_step = 10;
    let mut percent_done = print_step;
    if let Some(Arrays::ArrayTwo(amplifier_data)) = data.get_mut("amplifier_data") {
        let num_amplifier_channels = amplifier_data.shape()[0];
        for i in 0..num_amplifier_channels {
            let channel_data: Vec<f64> = amplifier_data.slice_mut(s![i, ..]).iter().map(|&x| x as f64).collect();
            if let (DataType::Float(sample_rate), DataType::Float(notch_filter_frequency)) = (&header["sample_rate"], &header["notch_filter_frequency"]) {
                let result = notch_filter(&channel_data, *sample_rate, *notch_filter_frequency, 10);
                amplifier_data.slice_mut(s![i, ..]).assign(&Array1::from(result.iter().map(|&x| x as i32).collect::<Vec<i32>>()));
            }

            percent_done = print_progress(i, num_amplifier_channels, print_step, percent_done);
        }
    }
}

fn notch_filter(signal_in: &Vec<f64>, f_sample: f32, f_notch: f32, bandwidth: i32) -> Vec<f64> {
    let t_step = 1.0 / f_sample;
    let f_c = f_notch * t_step;
    let signal_length = signal_in.len();
    let iir_parameters = calculate_iir_parameters(bandwidth, t_step, f_c);

    let mut signal_out = vec![0.0; signal_length];

    signal_out[0] = signal_in[0];
    signal_out[1] = signal_in[1];

    for i in 2..signal_length {
        signal_out[i] = calculate_iir(i, signal_in, &signal_out, &iir_parameters);
    }

    signal_out
}

fn calculate_iir_parameters(bandwidth: i32, t_step: f32, f_c: f32) -> HashMap<String, f64> {
    let f_c = f_c as f64;
    let bandwidth = bandwidth as f64;
    let t_step = t_step as f64; 
    let mut parameters = HashMap::new();
    let d = (-2.0 * PI * (bandwidth / 2.0) * t_step).exp();
    let b = (1.0 + d * d) * (2.0 * PI * f_c).cos();
    let a0 = 1.0;
    let a1 = -b;
    let a2 = d * d;
    let a = (1.0 + d * d) / 2.0;
    let b0 = 1.0;
    let b1 = -2.0 * (2.0 * PI * f_c).cos();
    let b2 = 1.0;

    parameters.insert("d".to_string(), d);
    parameters.insert("b".to_string(), b);
    parameters.insert("a0".to_string(), a0);
    parameters.insert("a1".to_string(), a1);
    parameters.insert("a2".to_string(), a2);
    parameters.insert("a".to_string(), a);
    parameters.insert("b0".to_string(), b0);
    parameters.insert("b1".to_string(), b1);
    parameters.insert("b2".to_string(), b2);

    parameters
}

fn calculate_iir(i: usize, signal_in: &Vec<f64>, signal_out: &Vec<f64>, iir_parameters: &HashMap<String, f64>) -> f64 {
    let sample = (
        iir_parameters["a"] * iir_parameters["b2"] * signal_in[i - 2]
        + iir_parameters["a"] * iir_parameters["b1"] * signal_in[i - 1]
        + iir_parameters["a"] * iir_parameters["b0"] * signal_in[i]
        - iir_parameters["a2"] * signal_out[i - 2]
        - iir_parameters["a1"] * signal_out[i - 1]
    ) / iir_parameters["a0"];

    sample
}

fn print_progress(current: usize, total: usize, step: usize, percent_done: usize) -> usize {
    let progress = (current as f64 / total as f64) * 100.0;
    if progress >= percent_done as f64 {
        println!("{}% done...", percent_done);
        return percent_done + step;
    }
    percent_done
}


#[derive(Debug)]
pub enum RhsError {
    UnrecognizedFileError,
    UnknownChannelTypeError,
    FileSizeError,
    QStringError,
    ChannelNotFoundError,
}

impl std::fmt::Display for RhsError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RhsError::UnrecognizedFileError => write!(f, "Invalid magic number, not an RHS header file"),
            RhsError::UnknownChannelTypeError => write!(f, "Channel field in RHS header has an unrecognized signal_type value"),
            RhsError::FileSizeError => write!(f, "File reading failed due to invalid file size"),
            RhsError::QStringError => write!(f, "Reading a QString failed because it is too long"),
            RhsError::ChannelNotFoundError => write!(f, "Specified channel not found when plotting"),
        }
    }
}

impl std::error::Error for RhsError {}
