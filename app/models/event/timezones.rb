class Event < ActiveRecord::Base

  def starts_at=(starts_at_time)
    write_attribute(:starts_at, starts_at_time)
  end

  def starts_at_local
    return starts_at unless timezone 
    starts_at.in_time_zone(timezone).to_datetime
  end

  def ends_at_local
    return ends_at unless timezone 
    ends_at.in_time_zone(timezone).to_datetime
  end


  def check_timezone
    set_timezone_from_calendar if self.calendar
    set_timezone_from_location if geocoded?
  end

  def start
    starts_at_local
  end

  def end
    ends_at_local
  end


  def set_timezone_from_location
    #RestClient.log = '/tmp/restclient.log'
  
    #TODO
    # extract this in do a delayed job
    tz_json = RestClient.post("http://ws.geonames.org/timezoneJSON", {'lat'=> latitude, 'lng'=>longitude})
    tz_parsed = JSON.parse(tz_json)
  
    return logger.warn( tz_parsed ) if tz_parsed['status']
    return set_timezone_from_tzinfo( tz_parsed['timezoneId'] ) if tz_parsed['timezoneId']
  
    set_timezone_from_offsets(tz_parsed['rawOffset'])
  end

  def set_timezone_from_tzinfo(tz_name)
    write_attribute(:timezone, tz_name)
    tz_info = TZInfo::Timezone.get(tz_name)

    write_attribute(:utc_offset, starts_at.in_time_zone(tz_info).utc_offset)
  end

  def set_timezone_from_offsets(raw_offset)
    #close enough
    write_attribute(:timezone, raw_offset.to_s + ":00")
    write_attribute(:utc_offset, raw_offset*3600)
  end

  def set_timezone_from_calendar
    return nil unless calendar && calendar.timezone && calendar.utc_offset
    write_attribute(:timezone, calendar.timezone) 
    write_attribute(:utc_offset, calendar.utc_offset) 
  end

end