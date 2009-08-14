class Event < ActiveRecord::Base
  belongs_to :calendar
  belongs_to :location
  before_save :geocode!
  before_save :check_timezone
  
  #need to add catches for events which end within the span but dont' start, or which
  #neither start nor end within it, but are subsets of the span. ugh
  named_scope :on_day, lambda { |day|
    { :conditions => 
        [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
          day, day + 1.day, day, day ], 
      :order => 'starts_at' }
    }
  
  named_scope :in_month, lambda { |month|
    { :conditions => 
      [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
        month, month + 1.month, month, month ], 
      :order => 'starts_at' }
    }  

  named_scope :between, lambda { |starts,ends|
    { :conditions => 
      [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
        starts, ends, starts, starts ], 
      :order => 'starts_at' }
    }  
  

#events which start before and ends after start
# starts during 


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
  
  
  def to_public_json
    self.to_json(:methods => [:start, :end, :starts_at_local, :ends_at_local])
  end
  
  #check to see if an event has been geocoded
  def geocoded?
    latitude && longitude
  end
  
  def geocode!
    #self.attributes= GeoRiot::geocode_with_placemaker(self.attributes)
    self.attributes= GeoRiot::geocode_with_geonames(self.attributes)
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
